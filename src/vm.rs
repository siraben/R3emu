use crate::alu::*;
use crate::bus::{Bus, BusRequest, BusResponse};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreType {
    F = 0,
    S = 1,
    M = 2,
}

#[derive(Clone, Copy)]
enum Pipeline {
    Execute,
    Load { register: u8 },
    Store { address: u16, value: u32 },
}

/// Native R3 control-state bits after an execution-unit slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ExecutionState {
    Execute = 1,
    Load = 2,
    Store = 4,
    Halted = 8,
}

/// Post-slot observation; request is the incoming bus transaction.
#[derive(Clone, Copy, Debug)]
pub struct SlotTrace {
    pub core: usize,
    pub pc: u16,
    pub state: ExecutionState,
    /// Native bit order: carry, overflow, zero, sign (least significant first).
    pub flags: u8,
    pub request: BusRequest,
    pub waited: bool,
}

pub struct TraceEntry {
    pub instruction: u32,
    pub addr: u32,
    pub ops: [u32; 3],
}

pub struct Trace {
    pub entries: Vec<TraceEntry>,
    pub max_size: usize,
}

#[derive(Clone, Copy)]
struct MulInfo {
    instruction: u32,
}

impl MulInfo {
    fn permits_secondary(self, instruction: u32) -> bool {
        // The S unit reuses the preceding M unit's product. Hardware compares
        // operand encodings, not values; destination and multiply variant may
        // differ. Even an otherwise harmless alias (including r0) is rejected.
        let same_operands = (self.instruction ^ instruction) & 0x41FE_FFFF == 0;
        let destination = (self.instruction >> 25) & 31;
        let primary = (self.instruction >> 20) & 31;
        let secondary = self.instruction & 31;
        let immediate = self.instruction & (1 << 30) != 0;
        same_operands && destination != primary && (immediate || destination != secondary)
    }
}

pub struct Vm {
    pub bus: Bus,
    regs: [u32; 32],
    flags: u8,
    pub ip: u32,
    pub halted: bool,
    pub cores: Vec<CoreType>,
    pub allow_smul: bool,
    pipeline: Pipeline,
    request: BusRequest,
    prefetched: Option<u32>,
    previous_multiply: Option<MulInfo>,
    pub trace: Option<Trace>,
}

impl Vm {
    pub fn new(
        bus: Bus,
        cores: Vec<CoreType>,
        allow_smul: bool,
        trace_size: Option<usize>,
    ) -> Self {
        assert!(
            !cores.is_empty(),
            "CPU must have at least one execution unit"
        );
        bus.validate_topology(cores.len());
        let trace = trace_size.map(|max_size| Trace {
            entries: Vec::with_capacity(max_size.min(1024 * 1024)),
            max_size,
        });
        Vm {
            bus,
            regs: [0u32; 32],
            flags: 0,
            ip: 0,
            halted: false,
            cores,
            allow_smul,
            pipeline: Pipeline::Execute,
            request: BusRequest::Read { address: 0 },
            prefetched: None,
            previous_multiply: None,
            trace,
        }
    }

    #[inline(always)]
    fn read_reg(&self, index: u8) -> u32 {
        let i = (index & 31) as usize;
        if i == 0 {
            0x20000000
        } else {
            self.regs[i]
        }
    }

    #[inline(always)]
    fn write_reg(&mut self, index: u8, val: u32) {
        let i = (index & 31) as usize;
        if i != 0 {
            self.regs[i] = sanitize_write(val);
        }
    }

    pub fn state(&self) -> ExecutionState {
        if self.halted {
            return ExecutionState::Halted;
        }
        match self.pipeline {
            Pipeline::Execute => ExecutionState::Execute,
            Pipeline::Load { .. } => ExecutionState::Load,
            Pipeline::Store { .. } => ExecutionState::Store,
        }
    }

    pub fn registers(&self) -> [u32; 32] {
        std::array::from_fn(|index| self.read_reg(index as u8))
    }

    /// Restore register contents from a hardware snapshot; r0 remains fixed.
    pub fn initialize_registers(&mut self, registers: [u32; 32]) {
        for (index, value) in registers.into_iter().enumerate() {
            self.write_reg(index as u8, value);
        }
    }

    pub fn cycle(&mut self) {
        self.cycle_with_observer(|_| {});
    }

    /// Advance one native frame, observing every unit, including waits, memory
    /// service and halted slots. No execution logic is duplicated by the observer.
    pub fn cycle_with_observer(&mut self, mut observe: impl FnMut(SlotTrace)) {
        self.bus.begin_frame();
        // The physical feedback path carries the last unit's instruction and
        // product into the first unit of the next frame (including M -> S).
        let mut previous_multiply = self.previous_multiply.take();
        for core in 0..self.cores.len() {
            let request = self.request;
            let (multiply, waited) = self.advance_slot(core, previous_multiply);
            previous_multiply = multiply;
            observe(SlotTrace {
                core,
                pc: self.ip as u16,
                state: self.state(),
                flags: ((self.flags & 3) << 2) | ((self.flags >> 2) & 3),
                request,
                waited,
            });
        }
        self.previous_multiply = previous_multiply;
        self.bus.end_frame();
    }

    fn advance_slot(
        &mut self,
        core: usize,
        previous_multiply: Option<MulInfo>,
    ) -> (Option<MulInfo>, bool) {
        let response = self.bus.transact(core, self.cores.len(), self.request);
        if response == BusResponse::Wait {
            return (None, true);
        }
        if self.halted {
            return (None, false);
        }
        match self.pipeline {
            Pipeline::Load { register } => {
                let BusResponse::Read(value) = response else {
                    unreachable!("load service must receive a read response")
                };
                self.write_reg(register, value);
                self.pipeline = Pipeline::Execute;
                self.request = BusRequest::Read {
                    address: self.ip as u16,
                };
                (None, false)
            }
            Pipeline::Store { address, value } => {
                let BusResponse::Read(instruction) = response else {
                    unreachable!("store service must receive the prefetched instruction")
                };
                // Fetch BEFORE the write reaches RAM. Keep this instruction
                // through bus waits, even if the store modifies its RAM cell.
                self.prefetched = Some(instruction);
                self.pipeline = Pipeline::Execute;
                self.request = BusRequest::Write { address, value };
                (None, false)
            }
            Pipeline::Execute => {
                let instruction = match self.prefetched.take() {
                    Some(instruction) => instruction,
                    None => {
                        let BusResponse::Read(instruction) = response else {
                            unreachable!("instruction fetch must receive a read response")
                        };
                        instruction
                    }
                };
                let multiply = self.exec_instruction(core, instruction, previous_multiply);
                if !matches!(self.pipeline, Pipeline::Load { .. }) {
                    self.request = BusRequest::Read {
                        address: self.ip as u16,
                    };
                }
                (multiply, false)
            }
        }
    }

    fn exec_instruction(
        &mut self,
        core_idx: usize,
        instruction: u32,
        prev_mul: Option<MulInfo>,
    ) -> Option<MulInfo> {
        self.ip &= 0xFFFF;

        // Decode fields
        let moi = instruction >> 31;
        let soii = (instruction >> 30) & 1;
        let destreg = ((instruction >> 25) & 0b11111) as u8;
        let psrcreg = ((instruction >> 20) & 0b11111) as u8;
        let loi = ((instruction >> 16) & 0b1111) as u8;
        let raw_ssrc = (instruction & 0xFFFF) as u16;

        // Resolve ssrcreg: if soii=0, read from register (truncated to u16)
        let ssrc_reg = (raw_ssrc as u8) & 31;
        let ssrc: u16 = if soii == 0 {
            self.read_reg(ssrc_reg) as u16
        } else {
            raw_ssrc
        };

        // Record trace entry
        let trace_psrc = self.read_reg(psrcreg);
        if let Some(ref mut trace) = self.trace {
            if trace.entries.len() < trace.max_size {
                trace.entries.push(TraceEntry {
                    instruction,
                    addr: self.ip,
                    ops: [destreg as u32, trace_psrc, ssrc as u32],
                });
            }
        }

        // Jump condition decoding
        let sync = (psrcreg >> 4) == 0;
        let condindex = (psrcreg & 0b1111) as usize;

        let core_type = self.cores[core_idx];
        let update = moi != 0;
        // The ISA leaves carry/overflow unspecified for these operations, but
        // R316 still exposes its parallel adder's outputs. Preserve that native
        // behavior, including the previous carry input for odd opcodes.
        let incidental_flags = if update && matches!(loi, 0 | 3 | 8 | 9 | 11 | 12) {
            let mut flags = self.flags;
            let carry = loi & 1 != 0 && get_flag(flags, 2);
            if loi & 2 == 0 {
                sbb(u32::from(ssrc), trace_psrc, &mut flags, carry, true);
            } else {
                adc(
                    trace_psrc,
                    u32::from(ssrc),
                    &mut flags,
                    u8::from(carry),
                    true,
                );
            }
            Some(flags & 0b1100)
        } else {
            None
        };
        let mut skip_ip_inc = false;
        let mut executed_mul: Option<MulInfo> = None;

        match loi {
            4 => {
                // sub: operand order is ssrc - psrcreg (reversed vs add)
                let psrc = self.read_reg(psrcreg);
                let result = sub(ssrc as u32, psrc, &mut self.flags, update);
                let result = (psrc & 0xFFFF0000) | (result & 0xFFFF);
                self.write_reg(destreg, result);
            }
            5 => {
                // sbb
                let psrc = self.read_reg(psrcreg);
                let carry_in = get_flag(self.flags, 2);
                let result = sbb(ssrc as u32, psrc, &mut self.flags, carry_in, update);
                let result = (psrc & 0xFFFF0000) | (result & 0xFFFF);
                self.write_reg(destreg, result);
            }
            6 => {
                // add
                let psrc = self.read_reg(psrcreg);
                let result = add(psrc, ssrc as u32, &mut self.flags, update);
                let result = (psrc & 0xFFFF0000) | (result & 0xFFFF);
                self.write_reg(destreg, result);
            }
            7 => {
                // adc
                let psrc = self.read_reg(psrcreg);
                let carry_in = if get_flag(self.flags, 2) { 1 } else { 0 };
                let result = adc(psrc, ssrc as u32, &mut self.flags, carry_in, update);
                let result = (psrc & 0xFFFF0000) | (result & 0xFFFF);
                self.write_reg(destreg, result);
            }
            8 => {
                // xor
                let psrc = self.read_reg(psrcreg);
                let result = xor(psrc, ssrc as u32, &mut self.flags, update);
                let result = (psrc & 0xFFFF0000) | (result & 0xFFFF);
                self.write_reg(destreg, result);
            }
            9 => {
                // or
                let psrc = self.read_reg(psrcreg);
                let result = or(psrc, ssrc as u32, &mut self.flags, update);
                let result = (psrc & 0xFFFF0000) | (result & 0xFFFF);
                self.write_reg(destreg, result);
            }
            11 => {
                // bitshift: raw_ssrc bit 15 determines direction
                let psrc = self.read_reg(psrcreg);
                let is_shr = (raw_ssrc >> 15) != 0;
                let result = if !is_shr {
                    shl(psrc, (ssrc as u32) & 0b1111, &mut self.flags, update)
                } else {
                    shr(psrc, (ssrc as u32) & 0b1111, &mut self.flags, update)
                };
                let result = (psrc & 0xFFFF0000) | (result & 0xFFFF);
                self.write_reg(destreg, result);
            }
            12 => {
                // and
                let psrc = self.read_reg(psrcreg);
                let result = and(psrc, ssrc as u32, &mut self.flags, update);
                let result = (psrc & 0xFFFF0000) | (result & 0xFFFF);
                self.write_reg(destreg, result);
            }
            13 => {
                // hlt
                self.halted = true;
            }
            14 => {
                // mul (moi=0) / muls (moi=1, M-core only)
                let psrc = self.read_reg(psrcreg);
                let can_mul = if moi == 0 {
                    match core_type {
                        CoreType::M => true,
                        CoreType::S => {
                            if !self.allow_smul {
                                false
                            } else {
                                prev_mul
                                    .is_some_and(|previous| previous.permits_secondary(instruction))
                            }
                        }
                        CoreType::F => false,
                    }
                } else {
                    core_type == CoreType::M
                };
                if can_mul {
                    let result = if moi == 0 {
                        mul(psrc, ssrc as u32)
                    } else {
                        muls(psrc, ssrc as u32)
                    };
                    let result = (psrc & 0xFFFF0000) | (result & 0xFFFF);
                    self.write_reg(destreg, result);
                    if core_type == CoreType::M {
                        executed_mul = Some(MulInfo { instruction });
                    }
                } else {
                    skip_ip_inc = true;
                }
            }
            15 => {
                // mulh (moi=0, M-core only) / mulx (moi=1, M-core only)
                let psrc = self.read_reg(psrcreg);
                let can_mul = core_type == CoreType::M;
                if can_mul {
                    let result = if moi == 0 {
                        mulh(psrc, ssrc as u32)
                    } else {
                        mulx(psrc, ssrc as u32)
                    };
                    let result = (psrc & 0xFFFF0000) | (result & 0xFFFF);
                    self.write_reg(destreg, result);
                    executed_mul = Some(MulInfo { instruction });
                } else {
                    skip_ip_inc = true;
                }
            }
            1 => {
                // jmp
                let next_ip = self.ip.wrapping_add(1) & 0xFFFF;
                // As in the physical ALU, JMP preserves the primary register's
                // upper half, even though its index also encodes the condition.
                let upper = self.read_reg(psrcreg) & 0xFFFF_0000;
                self.write_reg(destreg, upper | next_ip);
                if eval_condition(self.flags, condindex)
                    && (!sync || core_idx != self.cores.len() - 1)
                {
                    skip_ip_inc = true;
                    self.ip = ssrc as u32;
                }
            }
            2 => {
                let address = (self.read_reg(psrcreg) as u16).wrapping_add(ssrc);
                self.pipeline = Pipeline::Load { register: destreg };
                self.request = BusRequest::Read { address };
            }
            10 => {
                self.pipeline = Pipeline::Store {
                    address: (self.read_reg(psrcreg) as u16).wrapping_add(ssrc),
                    value: self.read_reg(destreg),
                };
            }
            0 => {
                // mov (moi=0) / movf (moi=1)
                let upper = self.read_reg(psrcreg) & 0xFFFF0000;
                let lower = ssrc as u32 & 0xFFFF;
                let newval = upper | lower;
                self.write_reg(destreg, newval);
                if moi != 0 {
                    set_flag(&mut self.flags, 0, (newval & 0xFFFF) == 0);
                    set_flag(&mut self.flags, 1, (newval >> 15) & 1 != 0);
                }
            }
            3 => {
                // exh: D[31:16] = S[15:0], D[15:0] = P[31:16]
                let shifted = (ssrc as u32) << 16;
                let lower = self.read_reg(psrcreg) >> 16;
                let newval = shifted | lower;
                self.write_reg(destreg, newval);
                if moi != 0 {
                    set_flag(&mut self.flags, 0, (newval & 0xFFFF) == 0);
                    set_flag(&mut self.flags, 1, (newval >> 15) & 1 != 0);
                }
            }
            _ => {} // unreachable: loi is 4 bits, all 16 values covered
        }

        if let Some(flags) = incidental_flags {
            self.flags = (self.flags & 0b0011) | flags;
        }
        if !skip_ip_inc {
            self.ip = (self.ip + 1) & 0xFFFF;
        }
        executed_mul
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::Bus;

    fn encode(moi: u32, soii: u32, dest: u8, psrc_or_cond: u8, op_lsb: u8, ssrc: u16) -> u32 {
        (moi << 31)
            | (soii << 30)
            | ((dest as u32) << 25)
            | ((psrc_or_cond as u32) << 20)
            | ((op_lsb as u32) << 16)
            | ssrc as u32
    }

    fn make_vm(cores: Vec<CoreType>) -> Vm {
        let bus = Bus::new(4, 128, 1, 1, false);
        Vm::new(bus, cores, true, None)
    }

    #[test]
    fn test_jn_writes_link_even_when_not_taken() {
        let mut vm = make_vm(vec![CoreType::M]);
        let jn = encode(0, 1, 1, 0x18, 1, 0x1234);
        vm.bus.ram[0] = jn;
        vm.cycle();
        assert_eq!(vm.read_reg(1), 1);
        assert_eq!(vm.ip, 1);
    }

    #[test]
    fn test_sync_jump_only_on_non_last_core() {
        let sync_jmp = encode(0, 1, 1, 0x00, 1, 0x0002);
        let hlt = encode(0, 0, 0, 0, 13, 0);

        // Single core: sync jump should not jump.
        let mut vm = make_vm(vec![CoreType::M]);
        vm.bus.ram[0] = sync_jmp;
        vm.bus.ram[1] = hlt;
        vm.cycle();
        assert_eq!(vm.read_reg(1), 0x2000_0001);
        assert_eq!(vm.ip, 1);

        // Two cores: sync jump should jump on core 0, then core 1 halts.
        let mut vm = make_vm(vec![CoreType::M, CoreType::M]);
        vm.bus.ram[0] = sync_jmp;
        vm.bus.ram[2] = hlt;
        vm.cycle();
        assert!(vm.halted);
        assert_eq!(vm.read_reg(1), 0x2000_0001);
        assert_eq!(vm.ip, 3);
    }

    #[test]
    fn logical_flags_match_the_native_parallel_adder() {
        let mut vm = make_vm(vec![CoreType::M]);
        vm.flags = 0b1100; // Cf and Of set
        vm.write_reg(1, 0x00FF);
        vm.write_reg(2, 0x0F0F);
        let and_inst = encode(1, 0, 3, 1, 12, 2);
        vm.bus.ram[0] = and_inst;
        vm.cycle();
        // The circuit subtracts 0x00ff from 0x0f0f alongside the AND: no
        // carry or overflow. These bits are unspecified by the abstract ISA.
        assert_eq!(vm.flags & 0b1100, 0);
    }

    #[test]
    fn test_smul_allowed_when_prev_m_mul_matches() {
        let mut vm = make_vm(vec![CoreType::M, CoreType::S]);
        vm.write_reg(5, 0x1234);
        vm.write_reg(6, 0x00FF);

        let mulh = encode(0, 0, 4, 5, 15, 6);
        let mul = encode(0, 0, 7, 5, 14, 6);
        vm.bus.ram[0] = mulh;
        vm.bus.ram[1] = mul;

        vm.cycle();

        let expected = (0x1234u32 * 0x00FFu32) & 0xFFFF;
        assert_eq!(vm.read_reg(7) & 0xFFFF, expected);
        assert_eq!(vm.ip, 2);
    }

    #[test]
    fn secondary_multiply_can_follow_the_last_m_unit_across_a_frame() {
        let mut vm = make_vm(vec![CoreType::S, CoreType::M]);
        vm.write_reg(5, 5);
        vm.write_reg(6, 6);
        vm.bus.ram[0] = encode(0, 0, 4, 5, 15, 6);
        vm.bus.ram[1] = encode(0, 0, 7, 5, 14, 6);
        vm.bus.ram[2] = encode(0, 0, 0, 0, 13, 0);
        vm.cycle();
        assert_eq!(vm.ip, 1);
        vm.cycle();
        assert!(vm.halted);
        assert_eq!(vm.read_reg(7), 30);
    }

    #[test]
    fn secondary_multiply_may_write_its_own_input() {
        let mut vm = make_vm(vec![CoreType::M, CoreType::S]);
        vm.write_reg(5, 5);
        vm.write_reg(6, 6);
        vm.bus.ram[0] = encode(0, 0, 4, 5, 15, 6);
        vm.bus.ram[1] = encode(0, 0, 5, 5, 14, 6);
        vm.cycle();
        assert_eq!(vm.read_reg(5), 30);
        assert_eq!(vm.ip, 2);
    }

    #[test]
    fn secondary_multiply_requires_identical_operand_encodings() {
        let previous = MulInfo {
            instruction: encode(0, 0, 4, 5, 15, 6),
        };
        assert!(previous.permits_secondary(encode(0, 0, 5, 5, 14, 6)));
        assert!(!previous.permits_secondary(encode(0, 0, 7, 8, 14, 6)));
        assert!(!previous.permits_secondary(encode(0, 0, 7, 5, 14, 8)));
        assert!(!previous.permits_secondary(encode(0, 1, 7, 5, 14, 6)));
        // Bits unused by register addressing are still compared by the circuit.
        assert!(!previous.permits_secondary(encode(0, 0, 7, 5, 14, 38)));
    }

    #[test]
    fn secondary_multiply_rejects_previous_destination_aliases() {
        for destination in [5, 6] {
            let previous = MulInfo {
                instruction: encode(0, 0, destination, 5, 15, 6),
            };
            assert!(!previous.permits_secondary(encode(0, 0, 7, 5, 14, 6)));
        }
        let previous = MulInfo {
            instruction: encode(0, 1, 6, 5, 15, 6),
        };
        assert!(previous.permits_secondary(encode(0, 1, 7, 5, 14, 6)));
    }

    #[test]
    fn test_smul_rejected_on_operand_mismatch() {
        let mut vm = make_vm(vec![CoreType::M, CoreType::S]);
        vm.write_reg(4, 0xAAAA);
        vm.write_reg(5, 0x1234);
        vm.write_reg(6, 0x00FF);
        vm.write_reg(7, 0x5555);

        let mulh = encode(0, 0, 4, 5, 15, 6);
        let mul = encode(0, 0, 7, 5, 14, 4);
        vm.bus.ram[0] = mulh;
        vm.bus.ram[1] = mul;

        vm.cycle();

        assert_eq!(vm.read_reg(7) & 0xFFFF, 0x5555);
        assert_eq!(vm.ip, 1);
    }
}
