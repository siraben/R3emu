/// Patch special word values to zero.
/// Values 0x00000000, 0x40000000, 0x80000000, 0xC0000000 all map to 0.
/// These are exactly the values with lower 30 bits all zero.
#[inline(always)]
pub fn patch_word(w: u32) -> u32 {
    if w & 0x3FFFFFFF == 0 { 0 } else { w }
}

/// Mask a word to the lower 16 bits (ALU word width).
#[inline(always)]
pub fn alu_word_limit(w: u32) -> u32 {
    w & 0xFFFF
}

/// Read a single flag bit.
/// Index: 0=Zf, 1=Sf, 2=Cf, 3=Of.
#[inline(always)]
pub fn get_flag(flags: u8, index: u8) -> bool {
    (flags >> index) & 1 != 0
}

/// Set a single flag bit.
#[inline(always)]
pub fn set_flag(flags: &mut u8, index: u8, state: bool) {
    if state {
        *flags |= 1 << index;
    } else {
        *flags &= !(1 << index);
    }
}

#[inline(always)]
fn add_common(a: u32, b: u32, flags: &mut u8, carry_in: u8, update: bool) -> u32 {
    let a = alu_word_limit(patch_word(a));
    let b = alu_word_limit(patch_word(b));
    let out = a + b + carry_in as u32;
    let out = patch_word(out);
    if update {
        set_flag(flags, 0, alu_word_limit(out) == 0); // zero
        set_flag(flags, 1, (alu_word_limit(out) >> 15) != 0); // sign
        set_flag(flags, 2, out > 0xFFFF); // carry
        let a_sign = (a >> 15) & 1;
        let b_sign = (b >> 15) & 1;
        let out_sign = (alu_word_limit(out) >> 15) & 1;
        set_flag(flags, 3, a_sign == b_sign && out_sign != a_sign); // overflow
    }
    alu_word_limit(out)
}

#[inline]
pub fn add(a: u32, b: u32, flags: &mut u8, update: bool) -> u32 {
    add_common(a, b, flags, 0, update)
}

#[inline]
pub fn adc(a: u32, b: u32, flags: &mut u8, carry_in: u8, update: bool) -> u32 {
    add_common(a, b, flags, carry_in, update)
}

#[inline]
pub fn sub(a: u32, b: u32, flags: &mut u8, update: bool) -> u32 {
    let out = add_common(a, !b, flags, 1, update);
    if update {
        let carry = get_flag(*flags, 2);
        set_flag(flags, 2, !carry);
    }
    out
}

#[inline]
pub fn sbb(a: u32, b: u32, flags: &mut u8, carry_in: bool, update: bool) -> u32 {
    let out = add_common(a, !b, flags, if carry_in { 0 } else { 1 }, update);
    if update {
        let carry = get_flag(*flags, 2);
        set_flag(flags, 2, !carry);
    }
    out
}

#[inline]
pub fn mul(a: u32, b: u32) -> u32 {
    let a = alu_word_limit(patch_word(a));
    let b = alu_word_limit(patch_word(b));
    (a.wrapping_mul(b)) & 0xFFFF
}

#[inline]
pub fn mulh(a: u32, b: u32) -> u32 {
    let a = alu_word_limit(patch_word(a));
    let b = alu_word_limit(patch_word(b));
    (a.wrapping_mul(b)) >> 16
}

#[inline]
pub fn muls(a: u32, b: u32) -> u32 {
    let a = alu_word_limit(patch_word(a)) as u16 as i16 as i32;
    let b = alu_word_limit(patch_word(b)) as u16 as i16 as i32;
    let out = a.wrapping_mul(b);
    (out as u32) >> 16
}

#[inline]
pub fn mulx(a: u32, b: u32) -> u32 {
    let a = alu_word_limit(patch_word(a));
    let b = (alu_word_limit(patch_word(b)) as u16 as i16 as i32) as u32;
    let out = a.wrapping_mul(b);
    out >> 16
}

/// Shift left. Result is masked to 16 bits per the ALU word width.
#[inline]
pub fn shl(target: u32, pos: u32, flags: &mut u8, update: bool) -> u32 {
    let target = patch_word(target);
    let pos = patch_word(pos) & 0b1111;
    let out = alu_word_limit(target << pos);
    let out = patch_word(out);
    if update {
        set_flag(flags, 0, out == 0);
        set_flag(flags, 1, (alu_word_limit(out) >> 15) != 0);
    }
    out
}

/// Shift right. Target is masked to 16 bits before shifting per ALU word width.
#[inline]
pub fn shr(target: u32, pos: u32, flags: &mut u8, update: bool) -> u32 {
    let target = alu_word_limit(patch_word(target));
    let pos = patch_word(pos) & 0b1111;
    let out = target >> pos;
    let out = patch_word(out);
    if update {
        set_flag(flags, 0, out == 0);
        set_flag(flags, 1, (alu_word_limit(out) >> 15) != 0);
    }
    out
}

#[inline]
pub fn and(a: u32, b: u32, flags: &mut u8, update: bool) -> u32 {
    let a = patch_word(a);
    let b = patch_word(b);
    let out = patch_word(a & b);
    if update {
        set_flag(flags, 2, false); // carry cleared
        set_flag(flags, 0, out == 0);
        set_flag(flags, 1, (alu_word_limit(out) >> 15) != 0);
    }
    out
}

#[inline]
pub fn or(a: u32, b: u32, flags: &mut u8, update: bool) -> u32 {
    let a = patch_word(a);
    let b = patch_word(b);
    let out = patch_word(a | b);
    if update {
        set_flag(flags, 2, false);
        set_flag(flags, 0, out == 0);
        set_flag(flags, 1, (alu_word_limit(out) >> 15) != 0);
    }
    out
}

#[inline]
pub fn xor(a: u32, b: u32, flags: &mut u8, update: bool) -> u32 {
    let a = patch_word(a);
    let b = patch_word(b);
    let out = patch_word(a ^ b);
    if update {
        set_flag(flags, 2, false);
        set_flag(flags, 0, out == 0);
        set_flag(flags, 1, (alu_word_limit(out) >> 15) != 0);
    }
    out
}

/// Evaluate a single jump condition directly from flags.
#[inline(always)]
pub fn eval_condition(flags: u8, index: usize) -> bool {
    let cf = get_flag(flags, 2);
    let zf = get_flag(flags, 0);
    let sf = get_flag(flags, 1);
    let of = get_flag(flags, 3);

    let base = match index & 7 {
        0 => true,            // always
        1 => cf || zf,        // below/equal
        2 => sf ^ of,         // less
        3 => zf || (sf ^ of), // less/equal
        4 => sf,              // sign
        5 => zf,              // zero
        6 => of,              // overflow
        7 => cf,              // carry
        _ => unreachable!(),
    };
    if index < 8 { base } else { !base }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_basic() {
        let mut flags: u8 = 0;
        let result = add(1, 1, &mut flags, true);
        assert_eq!(result, 2);
        assert!(!get_flag(flags, 0)); // zf
        assert!(!get_flag(flags, 1)); // sf
        assert!(!get_flag(flags, 2)); // cf
        assert!(!get_flag(flags, 3)); // of
    }

    #[test]
    fn test_add_overflow_to_zero() {
        let mut flags: u8 = 0;
        let result = add(0xFFFF, 1, &mut flags, true);
        assert_eq!(result, 0);
        assert!(get_flag(flags, 0));  // zf
        assert!(!get_flag(flags, 1)); // sf
        assert!(get_flag(flags, 2));  // cf
        assert!(!get_flag(flags, 3)); // of
    }

    #[test]
    fn test_add_signed_overflow() {
        let mut flags: u8 = 0;
        // -32768 + (-1) in 16-bit signed = overflow
        let result = add((-32768i32) as u32, (-1i32) as u32, &mut flags, true);
        assert_eq!(result, 0b0111111111111111);
        assert!(!get_flag(flags, 0)); // zf
        assert!(!get_flag(flags, 1)); // sf
        assert!(get_flag(flags, 2));  // cf
        assert!(get_flag(flags, 3));  // of
    }

    #[test]
    fn test_sub_basic() {
        let mut flags: u8 = 0;
        let result = sub(3, 1, &mut flags, true);
        assert_eq!(result, 2);
        assert!(!get_flag(flags, 0)); // zf
        assert!(!get_flag(flags, 1)); // sf
        assert!(!get_flag(flags, 2)); // cf
        assert!(!get_flag(flags, 3)); // of
    }

    #[test]
    fn test_add_twos_complement() {
        let mut flags: u8 = 0;
        // add(2, (~1)+1) = add(2, 0xFFFFFFFF)
        let result = add(2, (!1u32).wrapping_add(1), &mut flags, true);
        assert_eq!(result, 1);
        assert!(!get_flag(flags, 0)); // zf
        assert!(!get_flag(flags, 1)); // sf
        assert!(get_flag(flags, 2));  // cf
        assert!(!get_flag(flags, 3)); // of
    }

    #[test]
    fn test_add_twos_complement_negative() {
        let mut flags: u8 = 0;
        // add(2, (~4)+1) = add(2, 0xFFFFFFFC)
        let result = add(2, (!4u32).wrapping_add(1), &mut flags, true);
        assert_eq!(result, 65534);
        assert!(!get_flag(flags, 0)); // zf
        assert!(get_flag(flags, 1));  // sf
        assert!(!get_flag(flags, 2)); // cf
        assert!(!get_flag(flags, 3)); // of
    }

    #[test]
    fn test_add_wrap_around() {
        let mut flags: u8 = 0;
        let result = add(1, 65535, &mut flags, true);
        assert_eq!(result, 0);
        assert!(get_flag(flags, 0));  // zf
        assert!(!get_flag(flags, 1)); // sf
        assert!(get_flag(flags, 2));  // cf
        assert!(!get_flag(flags, 3)); // of
    }

    #[test]
    fn test_sub_borrow() {
        let mut flags: u8 = 0;
        let result = sub(1, 4, &mut flags, true);
        assert_eq!(result, 0b1111111111111101);
        assert!(!get_flag(flags, 0)); // zf
        assert!(get_flag(flags, 1));  // sf
        assert!(get_flag(flags, 2));  // cf (borrow)
        assert!(!get_flag(flags, 3)); // of
    }

    #[test]
    fn test_shl_basic() {
        let mut flags: u8 = 0;
        let result = shl(1, 1, &mut flags, true);
        assert_eq!(result, 2);
        assert!(!get_flag(flags, 0)); // zf
        assert!(!get_flag(flags, 1)); // sf
    }

    #[test]
    fn test_shl_larger() {
        let mut flags: u8 = 0;
        let result = shl(0x5, 2, &mut flags, true);
        assert_eq!(result, 20);
        assert!(!get_flag(flags, 0)); // zf
        assert!(!get_flag(flags, 1)); // sf
    }

    #[test]
    fn test_shr_basic() {
        let mut flags: u8 = 0;
        let result = shr(6, 2, &mut flags, true);
        assert_eq!(result, 1);
        assert!(!get_flag(flags, 0)); // zf
        assert!(!get_flag(flags, 1)); // sf
    }
}
