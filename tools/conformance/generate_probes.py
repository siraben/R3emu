#!/usr/bin/env python3
"""Generate focused ROMs for independent native conformance captures."""

import argparse
import json
from pathlib import Path
import struct


def instruction(op, dest=0, primary=0, secondary=0, *, immediate=True, modifier=False):
    return (
        int(modifier) << 31
        | int(immediate) << 30
        | dest << 25
        | primary << 20
        | op << 16
        | secondary
    )


def move(dest, value):
    return instruction(0, dest, secondary=value)


def store(dest, address):
    return instruction(10, dest, secondary=address)


def write(address, value):
    return [move(1, value), store(1, address)]


def write_word(address, value):
    return [
        move(1, value >> 16),
        instruction(3, 1, 0, 1, immediate=False),
        instruction(0, 1, 1, value & 0xFFFF),
        store(1, address),
    ]


def multiply(op, dest, primary, secondary, **kwargs):
    return instruction(op, dest, primary, secondary, immediate=False, **kwargs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    manifest = {}

    def emit(name, words, description, frames=100):
        words = words + [instruction(13, immediate=False)]
        path = args.out / (name + ".bin")
        path.write_bytes(struct.pack("<" + "I" * len(words), *words))
        manifest[name] = dict(
            description=description,
            frames=frames,
            words=[f"{word:08x}" for word in words],
        )

    # Each bank occupies at most the whole 12x8 terminal. Explicit cursor writes
    # avoid automatic scrolling so every character cell remains independently
    # observable in the final image. Glyph zero is deliberately programmed later.
    for bank in range(3):
        words = write(0x9FC2, 11 << 5) + write(0x9FC3, 7 << 5) + write(0x9FC6, 15)
        for index in range(bank * 96, min((bank + 1) * 96, 256)):
            cell = index - bank * 96
            words += write(0x9FC4, cell % 12 | (cell // 12) << 5)
            words += write(0x9F85, index)
        emit(
            f"font-bank-{bank}",
            words,
            "Full character-ROM bank; glyph zero uses saved registers.",
            300,
        )

    words = write(0x9FC2, 11 << 5) + write(0x9FC3, 7 << 5) + write(0x9FC6, 15)
    for bit in range(64):
        odd = 0xFFFFFFFF ^ (1 << bit) if bit < 32 else 0xFFFFFFFF
        even = 0xFFFFFFFF ^ (1 << (bit - 32)) if bit >= 32 else 0xFFFFFFFF
        words += write_word(0x9FC0, odd) + write_word(0x9FC1, even)
        words += write(0x9FC4, bit % 12 | (bit // 12) << 5) + write(0x9F85, 0)
        # Allow every render stage to finish before programming the next bitmap.
        for _ in range(3):
            words.append(instruction(1, secondary=len(words)))
    emit(
        "glyph-walking-clear",
        words,
        "Each of 64 custom-register bits cleared independently, with rendering barriers.",
        600,
    )

    # Synchronizing jump at PC 1 finishes on unit 9, placing the following M/S
    # pair on units 0/1 of the next frame despite the setup instruction.
    sync = instruction(1, secondary=1)
    emit(
        "fusion-equal-values-different-registers",
        [
            move(8, 5),
            sync,
            multiply(15, 4, 5, 6),
            multiply(14, 7, 8, 6),
            store(7, 1000),
        ],
        "Different encoded primary registers must stall S even when both hold 5.",
    )
    emit(
        "fusion-previous-clobber-zero",
        [
            move(5, 0),
            sync,
            multiply(15, 5, 5, 6),
            multiply(14, 7, 5, 6),
            store(7, 1000),
        ],
        "Previous M destination aliases its input, even though its value remains zero.",
    )
    emit(
        "fusion-unused-register-bits",
        [multiply(15, 4, 5, 6), multiply(14, 7, 5, 38), store(7, 1000)],
        "Full secondary encoding participates in fusion, including unused register bits.",
    )
    emit(
        "fusion-mode-mismatch",
        [multiply(15, 4, 5, 6), instruction(14, 7, 5, 6), store(7, 1000)],
        "Immediate 6 and register r6 holding 6 are not identical operand encodings.",
    )
    emit(
        "fusion-zero-register-alias",
        [multiply(15, 0, 0, 6), multiply(14, 7, 0, 6), store(7, 1000)],
        "The previous destination r0 conflicts even though writes to it are discarded.",
    )
    emit(
        "fusion-signed-high",
        [multiply(14, 4, 5, 6, modifier=True), multiply(14, 5, 5, 6), store(5, 1000)],
        "Signed-high M product can feed S low multiply, whose destination may alias an input.",
    )

    # A command immediately after an automatic scroll must retry on the next
    # bus visit; this distinguishes a timed terminal from final-image emulation.
    setup = write(0x9FC2, 0) + write(0x9FC3, 0) + write(0x9FC4, 0) + write(0x9FC6, 15)
    emit(
        "terminal-busy",
        setup
        + write(0x9F95, 65)
        + write(0x9F95, 66)
        + write(0x9F95, 67)
        + write(0x9FE4, 0),
        "One-cell automatic scrolling followed by another character and a pixel command.",
        150,
    )
    emit(
        "keyboard-empty",
        [instruction(2, 2, secondary=0x9F80), store(2, 1000)],
        "Capture the complete native empty-keyboard word, not just its low byte.",
    )
    emit(
        "jump-link-upper",
        [
            move(16, 0x1234),
            instruction(3, 16, 0, 16, immediate=False),
            instruction(1, 2, 16, 4),
            instruction(13),
            store(2, 1000),
        ],
        "JMP link upper half comes from the register selected by its condition field.",
    )
    emit(
        "glyph-register-race",
        write(0x9FC6, 15)
        + write(0x9FC4, 0)
        + write_word(0x9FC0, 0x55555555)
        + write_word(0x9FC1, 0xFFFFFFFF)
        + write(0x9F85, 0)
        + write_word(0x9FC0, 0xAAAAAAAA),
        "Reprogram a glyph bank in the frame after a print request.",
    )
    emit(
        "terminal-busy-read",
        setup
        + write(0x9F95, 65)
        + write(0x9F95, 66)
        + [instruction(2, 2, secondary=0x9F80), store(2, 1000)],
        "Keyboard input is a separate bus responder during a display retry.",
        150,
    )
    emit(
        "unserved-write",
        [move(1, 123), store(1, 0xA000)],
        "An unserved external write must retain its transaction and never reach HLT.",
    )
    emit(
        "load-frame-boundary",
        [move(0, 0)] * 9
        + [instruction(2, 1, secondary=500), instruction(6, 1, 1, 1), store(1, 1000)],
        "Load issued by unit 9 is serviced by unit 0 of the next frame.",
    )
    emit(
        "store-frame-boundary",
        [move(0, 0)] * 9 + [store(1, 1000)],
        "Store decode, prefetch and acknowledgement cross a frame boundary.",
    )
    for horizontal in (False, True):
        primary, secondary = 3 | (1 << 5), 2
        hrange, vrange = (primary, secondary) if horizontal else (secondary, primary)
        cursor = 3 | (2 << 5) if horizontal else 2 | (3 << 5)
        words = write(0x9FC2, hrange) + write(0x9FC3, vrange) + write(0x9FC4, cursor)
        words += write(0x9FC6, 15) + write(0x9FC5, ord("!"))
        address = 0x9FB5 if horizontal else 0x9FB1
        for character in b"ABCD!EFGHIJKL":
            words += write(address, character)
        emit(
            "terminal-reverse-" + ("row" if horizontal else "column"),
            words,
            "Reversed cursor ranges, nonblank newline glyph and automatic scrolling.",
            150,
        )
    words = []
    checks = []
    edge_pairs = [
        (0, 0),
        (0xFFFF, 1),
        (0x7FFF, 1),
        (0x8000, 0xFFFF),
        (1, 0xFFFF),
        (0x8000, 1),
        (0xFFFF, 0xFFFF),
        (12345, 45678),
    ]
    for operation in ("adc", "sbb"):
        for carry in (0, 1):
            for a, b in edge_pairs:
                words += [
                    move(1, a),
                    move(2, b),
                    move(4, 0xFFFF if carry else 0),
                    instruction(6, 0, 4, carry, modifier=True),
                    instruction(
                        7 if operation == "adc" else 5,
                        3,
                        1,
                        2,
                        immediate=False,
                        modifier=True,
                    ),
                ]
                sa, sb = (a if a < 32768 else a - 65536), (
                    b if b < 32768 else b - 65536
                )
                result = a + b + carry if operation == "adc" else b - a - carry
                signed = sa + sb + carry if operation == "adc" else sb - sa - carry
                address = 1400 + len(checks)
                words.append(store(3, address))
                checks.append(
                    dict(
                        op=operation,
                        a=a,
                        b=b,
                        carry=carry,
                        address=address,
                        expected=0x20000000 | (result & 0xFFFF),
                    )
                )
                flags = [
                    result & 0xFFFF == 0,
                    bool(result & 0x8000),
                    not 0 <= result <= 65535,
                    not -32768 <= signed <= 32767,
                ]
                for flag, condition, value in zip("ZSCO", [5, 4, 7, 6], flags):
                    base = len(words)
                    address = 1400 + len(checks)
                    words += [
                        move(4, 0),
                        instruction(1, 0, 16 + condition, base + 3),
                        instruction(1, 0, 16, base + 4),
                        move(4, 1),
                        store(4, address),
                    ]
                    checks.append(
                        dict(
                            op=operation,
                            a=a,
                            b=b,
                            carry=carry,
                            flag=flag,
                            address=address,
                            expected=0x20000000 | int(value),
                        )
                    )
    emit(
        "carry-matrix",
        words,
        "32 ADC/SBB results and all 128 flags, with both carry inputs.",
        500,
    )
    (args.out / "carry-matrix.json").write_text(json.dumps(checks, indent=2) + "\n")
    (args.out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"Generated {len(manifest)} ROMs in {args.out}")


if __name__ == "__main__":
    main()
