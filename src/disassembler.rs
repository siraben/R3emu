use crate::alu::patch_word;

const REG_NAMES: [&str; 33] = [
    "r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "r8", "r9", "r10",
    "r11", "r12", "r13", "r14", "r15", "r16", "r17", "r18", "r19", "r20",
    "r21", "r22", "r23", "r24", "r25", "r26", "r27", "r28", "r29", "r30",
    "r31", "rXX",
];

const JMP_NAMES: [&str; 16] = [
    "mp", "be", "l", "le", "s", "z", "o", "c", "n", "nbe", "nl", "nle",
    "ns", "nz", "no", "nc",
];

fn reg_name(idx: u8) -> &'static str {
    if idx <= 31 {
        REG_NAMES[idx as usize]
    } else {
        REG_NAMES[32]
    }
}

pub fn disassemble(instruction: u32) -> String {
    let moi = instruction >> 31;
    let soii = (instruction >> 30) & 1;
    let destreg = ((instruction >> 25) & 0b11111) as u8;
    let psrcreg = ((instruction >> 20) & 0b11111) as u8;
    let loi = ((instruction >> 16) & 0b1111) as u8;
    let raw_ssrc = (instruction & 0xFFFF) as u16;

    let ssrc_val = patch_word(raw_ssrc as u32) as u16;

    // For soii=1, display the immediate value; for soii=0, display register name
    if soii == 0 && ssrc_val > 31 {
        return format!("dw {}", instruction);
    }

    let ssrc_str = if soii != 0 {
        format!("{}", ssrc_val)
    } else {
        reg_name(ssrc_val as u8).to_string()
    };

    // Jump condition decoding
    let sync = (psrcreg >> 4) == 0;
    let condindex = (psrcreg & 0b1111) as usize;

    // Use raw_ssrc bit 15 for shift direction (matches instruction execution)
    let is_shr = (raw_ssrc >> 15) != 0;

    match loi {
        4 => {
            // sub
            let suffix = if moi == 0 { "subs" } else { "sub" };
            format!(
                "{} {}, {}, {}",
                suffix,
                reg_name(destreg),
                ssrc_str,
                reg_name(psrcreg)
            )
        }
        5 => {
            // sbb
            let suffix = if moi == 0 { "sbbs" } else { "sbb" };
            format!(
                "{} {}, {}, {}",
                suffix,
                reg_name(destreg),
                ssrc_str,
                reg_name(psrcreg)
            )
        }
        6 => {
            // add
            let suffix = if moi == 0 { "adds" } else { "add" };
            format!(
                "{} {}, {}, {}",
                suffix,
                reg_name(destreg),
                reg_name(psrcreg),
                ssrc_str
            )
        }
        7 => {
            // adc
            let suffix = if moi == 0 { "adcs" } else { "adc" };
            format!(
                "{} {}, {}, {}",
                suffix,
                reg_name(destreg),
                reg_name(psrcreg),
                ssrc_str
            )
        }
        8 => {
            // xor
            let suffix = if moi == 0 { "xors" } else { "xor" };
            format!(
                "{} {}, {}, {}",
                suffix,
                reg_name(destreg),
                reg_name(psrcreg),
                ssrc_str
            )
        }
        9 => {
            // or
            let suffix = if moi == 0 { "ors" } else { "or" };
            format!(
                "{} {}, {}, {}",
                suffix,
                reg_name(destreg),
                reg_name(psrcreg),
                ssrc_str
            )
        }
        11 => {
            // bitshift
            if !is_shr {
                let suffix = if moi == 0 { "shls" } else { "shl" };
                format!(
                    "{} {}, {}, {}",
                    suffix,
                    reg_name(destreg),
                    reg_name(psrcreg),
                    ssrc_str
                )
            } else {
                let suffix = if moi == 0 { "shrs" } else { "shr" };
                format!(
                    "{} {}, {}, {}",
                    suffix,
                    reg_name(destreg),
                    reg_name(psrcreg),
                    ssrc_str
                )
            }
        }
        12 => {
            // and
            let suffix = if moi == 0 { "ands" } else { "and" };
            format!(
                "{} {}, {}, {}",
                suffix,
                reg_name(destreg),
                reg_name(psrcreg),
                ssrc_str
            )
        }
        13 => {
            // hlt
            "hlt".to_string()
        }
        14 => {
            // mul / muls
            let name = if moi == 0 { "mul" } else { "muls" };
            format!(
                "{} {}, {}, {}",
                name,
                reg_name(destreg),
                reg_name(psrcreg),
                ssrc_str
            )
        }
        15 => {
            // mulh / mulx
            let name = if moi == 0 { "mulh" } else { "mulx" };
            format!(
                "{} {}, {}, {}",
                name,
                reg_name(destreg),
                reg_name(psrcreg),
                ssrc_str
            )
        }
        1 => {
            // jmp
            let mut name = String::from("j");
            if sync {
                name.push('y');
            }
            if condindex == 0 {
                if !sync {
                    name.push_str(JMP_NAMES[0]);
                }
            } else {
                name.push_str(JMP_NAMES[condindex]);
            }
            format!("{} {}, {}", name, reg_name(destreg), ssrc_str)
        }
        2 => {
            // ld
            format!(
                "ld {}, {}, {}",
                reg_name(destreg),
                reg_name(psrcreg),
                ssrc_str
            )
        }
        10 => {
            // st
            format!(
                "st {}, {}, {}",
                reg_name(destreg),
                reg_name(psrcreg),
                ssrc_str
            )
        }
        3 => {
            // exh
            format!(
                "exh {}, {}, {}",
                reg_name(destreg),
                reg_name(psrcreg),
                ssrc_str
            )
        }
        0 => {
            // mov / movf
            let name = if moi != 0 { "movf" } else { "mov" };
            format!(
                "{} {}, {}, {}",
                name,
                reg_name(destreg),
                reg_name(psrcreg),
                ssrc_str
            )
        }
        _ => format!("dw {}", instruction),
    }
}
