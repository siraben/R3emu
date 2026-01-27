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
            trace,
        }
    }

    #[inline(always)]
    fn read_reg(&self, index: u8) -> u32 {
        self.regs[(index & 31) as usize]
    }

    #[inline(always)]
    fn write_reg(&mut self, index: u8, val: u32) {
        let i = (index & 31) as usize;
        if i != 0 {
            self.regs[i] = patch_word(val);
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
                let val = self.read_reg(self.sch_reg);
                self.bus.write(self.sch_addr, val);
            }
            SchMode::Idle => {}
        }
        self.sch_mode = SchMode::Idle;
    }

    pub fn cycle(&mut self) {
        let core_count = self.cores.len();
        for i in 0..core_count {
            self.handle_sch_mem();
            if self.halted {
                break;
            }
            self.exec_instruction(i);
        }
        self.handle_sch_mem();
    }

    fn exec_instruction(&mut self, core_idx: usize) {
        // Reset IP if out of bounds
        let mem_size = self.bus.mem_size();
        if self.ip > mem_size as u32 {
            self.ip = 0;
        }

        // Fetch instruction
        let instruction = self.bus.read(self.ip as u16);

        // Decode fields
        let moi = instruction >> 31;
        let soii = (instruction >> 30) & 1;
        let destreg = ((instruction >> 25) & 0b11111) as u8;
        let psrcreg = ((instruction >> 20) & 0b11111) as u8;
        let loi = ((instruction >> 16) & 0b1111) as u8;
        let raw_ssrc = (instruction & 0xFFFF) as u16;

        // Resolve ssrcreg: if soii=0, read from register (truncated to u16)
        let ssrc: u16 = if soii == 0 {
            self.read_reg(raw_ssrc as u8) as u16
        } else {
            raw_ssrc
        };

        // Apply patchword (matches C behavior; effectively no-op for u16 values)
        let ssrc = patch_word(ssrc as u32) as u16;

        // Record trace entry
        if let Some(ref mut trace) = self.trace {
            if trace.entries.len() < trace.max_size {
                let psrc_val = self.regs[(psrcreg & 31) as usize];
                trace.entries.push(TraceEntry {
                    instruction,
                    addr: self.ip,
                    ops: [destreg as u32, psrc_val, ssrc as u32],
                });
            }
        }

        // Jump condition decoding
        let sync = (psrcreg >> 4) == 0;
        let condindex = (psrcreg & 0b1111) as usize;

        let core_type = self.cores[core_idx];
        let update = moi != 0;
        let mut skip_ip_inc = false;

        match loi {
            4 => {
                // sub: operand order is ssrc - psrcreg (reversed vs add)
                let result =
                    sub(ssrc as u32, self.read_reg(psrcreg), &mut self.flags, update);
                self.write_reg(destreg, result);
            }
            5 => {
                // sbb
                let carry_in = get_flag(self.flags, 2);
                let result = sbb(
                    ssrc as u32,
                    self.read_reg(psrcreg),
                    &mut self.flags,
                    carry_in,
                    update,
                );
                self.write_reg(destreg, result);
            }
            6 => {
                // add
                let result =
                    add(self.read_reg(psrcreg), ssrc as u32, &mut self.flags, update);
                self.write_reg(destreg, result);
            }
            7 => {
                // adc
                let carry_in = if get_flag(self.flags, 2) { 1 } else { 0 };
                let result = adc(
                    self.read_reg(psrcreg),
                    ssrc as u32,
                    &mut self.flags,
                    carry_in,
                    update,
                );
                self.write_reg(destreg, result);
            }
            8 => {
                // xor
                let result =
                    xor(self.read_reg(psrcreg), ssrc as u32, &mut self.flags, update);
                self.write_reg(destreg, result);
            }
            9 => {
                // or
                let result =
                    or(self.read_reg(psrcreg), ssrc as u32, &mut self.flags, update);
                self.write_reg(destreg, result);
            }
            11 => {
                // bitshift: raw_ssrc bit 15 determines direction
                let is_shr = (raw_ssrc >> 15) != 0;
                let result = if !is_shr {
                    shl(
                        self.read_reg(psrcreg),
                        (ssrc as u32) & 0b1111,
                        &mut self.flags,
                        update,
                    )
                } else {
                    shr(
                        self.read_reg(psrcreg),
                        (ssrc as u32) & 0b1111,
                        &mut self.flags,
                        update,
                    )
                };
                self.write_reg(destreg, result);
            }
            12 => {
                // and
                let result =
                    and(self.read_reg(psrcreg), ssrc as u32, &mut self.flags, update);
                self.write_reg(destreg, result);
            }
            13 => {
                // hlt
                self.halted = true;
            }
            14 => {
                // mul (moi=0) / muls (moi=1)
                let can_mul =
                    (core_type == CoreType::S && self.allow_smul) || core_type == CoreType::M;
                if can_mul {
                    let result = if moi == 0 {
                        mul(self.read_reg(psrcreg), ssrc as u32)
                    } else {
                        muls(self.read_reg(psrcreg), ssrc as u32)
                    };
                    self.write_reg(destreg, result);
                } else {
                    skip_ip_inc = true;
                }
            }
            15 => {
                // mulh (moi=0) / mulx (moi=1)
                let can_mul =
                    (core_type == CoreType::S && self.allow_smul) || core_type == CoreType::M;
                if can_mul {
                    let result = if moi == 0 {
                        mulh(self.read_reg(psrcreg), ssrc as u32)
                    } else {
                        mulx(self.read_reg(psrcreg), ssrc as u32)
                    };
                    self.write_reg(destreg, result);
                } else {
                    skip_ip_inc = true;
                }
            }
            1 => {
                // jmp
                if eval_condition(self.flags, condindex) {
                    skip_ip_inc = true;
                    // Sync jumps only execute on the last core
                    if !sync || core_idx == self.cores.len() - 1 {
                        self.write_reg(destreg, self.ip + 1);
                        self.ip = ssrc as u32;
                    }
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
                self.sch_reg = destreg;
            }
            0 => {
                // mov (moi=0) / movf (moi=1)
                let upper = self.read_reg(psrcreg) & 0xFFFF0000;
                let lower = ssrc as u32 & 0xFFFF;
                let newval = patch_word(upper | lower);
                self.write_reg(destreg, newval);
                if moi != 0 {
                    set_flag(&mut self.flags, 0, newval == 0);
                    set_flag(&mut self.flags, 1, (newval >> 31) != 0);
                    set_flag(&mut self.flags, 2, false);
                }
            }
            3 => {
                // exh
                let shifted = self.read_reg(psrcreg) << 16;
                let lower = (ssrc as u32) >> 16; // always 0 since ssrc is u16
                let newval = patch_word(shifted | lower);
                self.write_reg(destreg, newval);
                if moi != 0 {
                    set_flag(&mut self.flags, 0, newval == 0);
                    set_flag(&mut self.flags, 1, (newval >> 31) != 0);
                    set_flag(&mut self.flags, 2, false);
                }
            }
            _ => {} // unreachable: loi is 4 bits, all 16 values covered
        }

        if !skip_ip_inc {
            self.ip += 1;
        }
    }
}
