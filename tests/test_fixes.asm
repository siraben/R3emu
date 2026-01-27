; Test program for the 10 correctness fixes applied to R3emu.
; Tests fixes #1 (r0 value), #2 (shl limit), #3 (shr limit),
; #5 (exh encoding), #7 (movf/exhf 16-bit flags), #9 (ALU upper forwarding).
; Fixes #4 (sync jump), #6 (scrollprint bg), #8 (cursor wrap), #10 (mul gating)
; are validated indirectly by the bee_lzss end-to-end test.
;
; Expected stdout output with fixes:    PPPPPPP
; Expected stdout output without fixes: FFFFFFF

%include "common"

%define term_base 0x9F80
%eval term_term   term_base 0x35 +
%eval term_hrange term_base 0x42 +
%eval term_vrange term_base 0x43 +
%eval term_cursor term_base 0x44 +
%eval term_nlchar term_base 0x45 +
%eval term_colour term_base 0x46 +

; === Initialize terminal for stdout output ===
test_fixes:
st r0, term_cursor
mov r1, 11
shl r1, 5
st r1, term_hrange
mov r1, 7
shl r1, 5
st r1, term_vrange
mov r1, 0x0A
st r1, term_colour
mov r1, 10
st r1, term_nlchar

; Load a 32-bit instruction word from address 0 into r5.
; This word has non-zero upper 16 bits (from instruction encoding),
; providing upper bits independent of fix 1 for use in tests 3, 6, and 7.
ld r5, 0

; === Test 1: r0 returns 0x20000000 + exh extracts upper bits (fixes 1, 5) ===
; r0 should read as 0x20000000 (functionally zero with upper bits = 0x2000).
; mov r1, r0 gives r1 = 0x20000000.
; exh r2, r1, 0 extracts upper 16 of r1 into lower 16 of r2 = 0x2000.
mov r1, r0
exh r2, r1, 0
cmp r2, 0x2000
je .t1p
mov r3, 'F'
jmp .t1d
.t1p:
mov r3, 'P'
.t1d:
st r3, term_term

; === Test 2: shl result limited to 16 bits (fix 2) ===
; 0x8000 << 1 = 0x10000, which exceeds 16 bits.
; With fix: result is 0, zero flag set.
; Without fix: result is 0x10000 (not zero), zero flag clear.
mov r1, 0x8000
shl r1, 1
jz .t2p
mov r3, 'F'
jmp .t2d
.t2p:
mov r3, 'P'
.t2d:
st r3, term_term

; === Test 3: shr target limited to 16 bits (fix 3) ===
; r5 holds a 32-bit instruction word (upper 16 bits non-zero).
; mov r1, r5, 0: upper 16 from r5, lower 16 = 0.
; shr r1, 1: with fix, target masked to lower 16 (=0), result = 0, Zf=1.
; Without fix: target is full 32-bit (upper bits set), result != 0, Zf=0.
mov r1, r5, 0
shr r1, 1
jz .t3p
mov r3, 'F'
jmp .t3d
.t3p:
mov r3, 'P'
.t3d:
st r3, term_term

; === Test 4: exh puts S value into upper half (fix 5) ===
; exh r2, r1, 0xAB should put 0xAB in upper 16 of r2.
; Extract it back with another exh to verify.
mov r1, 0x1234
exh r2, r1, 0x00AB
exh r3, r2, 0
cmp r3, 0x00AB
je .t4p
mov r3, 'F'
jmp .t4d
.t4p:
mov r3, 'P'
.t4d:
st r3, term_term

; === Test 5: movf sets sign flag from bit 15 (fix 7) ===
; movf r1, 0xFFFF: bit 15 of lower 16 is 1, so Sf should be 1.
; Before fix: Sf = bit 31 of full 32-bit value = 0 (wrong).
movf r1, 0xFFFF
js .t5p
mov r3, 'F'
jmp .t5d
.t5p:
mov r3, 'P'
.t5d:
st r3, term_term

; === Test 6: movf sets zero flag from lower 16 bits (fix 7) ===
; movf r1, r5, 0: upper 16 from r5 (non-zero), lower 16 = 0.
; With fix 7: Zf = (lower 16 == 0) = 1 (correct).
; Without fix 7: Zf = (full 32-bit == 0) = 0 (wrong, upper bits non-zero).
movf r1, r5, 0
jz .t6p
mov r3, 'F'
jmp .t6d
.t6p:
mov r3, 'P'
.t6d:
st r3, term_term

; === Test 7: ALU upper 16 forwarding from P register (fix 9) ===
; add r1, r5, 0: should preserve upper 16 bits of r5 (P operand).
; exh r2, r1, 0: extract upper 16 into lower 16 of r2.
; With fix 9: upper preserved from r5, r2 != 0.
; Without fix 9: upper zeroed by ALU, r2 == 0.
add r1, r5, 0
exh r2, r1, 0
cmp r2, 0
jne .t7p
mov r3, 'F'
jmp .t7d
.t7p:
mov r3, 'P'
.t7d:
st r3, term_term

; === Done ===
mov r1, 10
st r1, term_term
hlt
