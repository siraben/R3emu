use crate::alu::*;
use crate::bus::Bus;

#[derive(Clone, Copy, PartialEq)]
pub enum CoreType {
    F = 0,
    S = 1,
    M = 2,
}

#[derive(Clone, Copy, PartialEq)]
enum SchMode {
    Load,
    Store,
    Idle,
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
    p_val: u16,
    s_val: u16,
}

pub struct Vm {
    pub bus: Bus,
    regs: [u32; 32],
    flags: u8,
    pub ip: u32,
    pub halted: bool,
    pub cores: Vec<CoreType>,
    pub allow_smul: bool,
    sch_addr: u16,
    sch_mode: SchMode,
    sch_reg: u8,
    sch_value: u32,
    pub trace: Option<Trace>,
}

impl Vm {
    pub fn new(
        bus: Bus,
        cores: Vec<CoreType>,
        allow_smul: bool,
        trace_size: Option<usize>,
    ) -> Self {
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
            sch_addr: 0,
            sch_mode: SchMode::Idle,
            sch_reg: 0,
            sch_value: 0,
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

    #[inline(always)]
    fn handle_sch_mem(&mut self) {
        match self.sch_mode {
            SchMode::Load => {
                let val = self.bus.read(self.sch_addr);
                self.write_reg(self.sch_reg, val);
            }
            SchMode::Store => {
                self.bus.write(self.sch_addr, self.sch_value);
            }
            SchMode::Idle => {}
        }
        self.sch_mode = SchMode::Idle;
    }

    pub fn cycle(&mut self) {
        let core_count = self.cores.len();
        let mut prev_mul: Option<MulInfo> = None;
        for i in 0..core_count {
            if self.halted {
                break;
            }
            prev_mul = self.exec_instruction(i, prev_mul);
        }
    }

    fn exec_instruction(&mut self, core_idx: usize, prev_mul: Option<MulInfo>) -> Option<MulInfo> {
        self.ip &= 0xFFFF;

        // Fetch instruction before applying pending memory ops (store writes are delayed by 1 fetch)
        let instruction = self.bus.read(self.ip as u16);
        self.handle_sch_mem();

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
                let p_val = alu_word_limit(psrc) as u16;
                let s_val = ssrc;
                let can_mul = if moi == 0 {
                    match core_type {
                        CoreType::M => true,
                        CoreType::S => {
                            if !self.allow_smul {
                                false
                            } else {
                                let operands_match = prev_mul
                                    .map(|m| m.p_val == p_val && m.s_val == s_val)
                                    .unwrap_or(false);
                                let dest_conflict =
                                    destreg == psrcreg || (soii == 0 && destreg == ssrc_reg);
                                operands_match && !dest_conflict
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
                        executed_mul = Some(MulInfo { p_val, s_val });
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
                    let p_val = alu_word_limit(psrc) as u16;
                    let s_val = ssrc;
                    let result = if moi == 0 {
                        mulh(psrc, ssrc as u32)
                    } else {
                        mulx(psrc, ssrc as u32)
                    };
                    let result = (psrc & 0xFFFF0000) | (result & 0xFFFF);
                    self.write_reg(destreg, result);
                    executed_mul = Some(MulInfo { p_val, s_val });
                } else {
                    skip_ip_inc = true;
                }
            }
            1 => {
                // jmp
                let next_ip = self.ip.wrapping_add(1) & 0xFFFF;
                self.write_reg(destreg, next_ip);
                if eval_condition(self.flags, condindex)
                    && (!sync || core_idx != self.cores.len() - 1)
                {
                    skip_ip_inc = true;
                    self.ip = ssrc as u32;
                }
            }
            2 => {
                // ld (scheduled for next handle_sch_mem)
                self.sch_mode = SchMode::Load;
                self.sch_addr =
                    (alu_word_limit(self.read_reg(psrcreg)) + alu_word_limit(ssrc as u32)) as u16;
                self.sch_reg = destreg;
            }
            10 => {
                // st (scheduled for next handle_sch_mem)
                self.sch_mode = SchMode::Store;
                self.sch_addr =
                    (alu_word_limit(self.read_reg(psrcreg)) + alu_word_limit(ssrc as u32)) as u16;
                self.sch_value = self.read_reg(destreg);
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
        assert_eq!(vm.read_reg(1), 1);
        assert_eq!(vm.ip, 1);

        // Two cores: sync jump should jump on core 0, then core 1 halts.
        let mut vm = make_vm(vec![CoreType::M, CoreType::M]);
        vm.bus.ram[0] = sync_jmp;
        vm.bus.ram[2] = hlt;
        vm.cycle();
        assert!(vm.halted);
        assert_eq!(vm.read_reg(1), 1);
        assert_eq!(vm.ip, 3);
    }

    #[test]
    fn test_logical_flags_do_not_clobber_cf_of() {
        let mut vm = make_vm(vec![CoreType::M]);
        vm.flags = 0b1100; // Cf and Of set
        vm.write_reg(1, 0x00FF);
        vm.write_reg(2, 0x0F0F);
        let and_inst = encode(1, 0, 3, 1, 12, 2);
        vm.bus.ram[0] = and_inst;
        vm.cycle();
        assert_eq!(vm.flags & 0b1100, 0b1100);
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
