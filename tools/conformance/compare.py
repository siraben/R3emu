#!/usr/bin/env python3
"""Compare production-VM traces to native TPT, aligned at observed CPU start."""

import argparse
import csv
import json
from pathlib import Path


def rows(path):
    with path.open() as stream:
        return list(csv.DictReader(stream))


def compare(native, emulator):
    native_rows = rows(native / "trace.csv")
    start = next(
        int(row["frame"]) for row in native_rows if int(row["state"]) & 15 == 1
    )
    actual = {
        (int(r["frame"]), int(r["core"])): r for r in rows(emulator / "slots.csv")
    }
    differences = {key: 0 for key in ("pc", "state", "flags", "ram_addr", "io_state")}
    first = {}
    masks = dict(pc=0xFFFF, state=15, flags=15, ram_addr=0xFFFFF, io_state=1)
    compared_slots = 0
    for expected in native_rows:
        frame, core = int(expected["frame"]) - start, int(expected["core"])
        if frame <= 0:
            continue
        observed = actual.get((frame, core))
        if observed is None:
            raise ValueError(f"Missing emulator slot at frame {frame}, core {core}")
        compared_slots += 1
        for field, mask in masks.items():
            e, a = int(expected[field]) & mask, int(observed[field]) & mask
            if e != a:
                differences[field] += 1
                first.setdefault(
                    field, dict(frame=frame, core=core, native=e, emulator=a)
                )
    expected_ram = (native / "final.bin").read_bytes()
    actual_ram = (emulator / "final.bin").read_bytes()
    if len(expected_ram) != len(actual_ram):
        raise ValueError("RAM lengths differ")
    ram_differences = sum(
        expected_ram[i : i + 4] != actual_ram[i : i + 4]
        for i in range(0, len(expected_ram), 4)
    )
    # PPM payload is sufficient for final comparisons even for older captures.
    expected_screen = (native / "screen-final.ppm").read_bytes().split(b"\n", 3)[3]
    actual_screen = (emulator / "screen-final.ppm").read_bytes().split(b"\n", 3)[3]
    if len(expected_screen) != len(actual_screen):
        raise ValueError("Framebuffer lengths differ")
    pixels = sum(
        expected_screen[i : i + 3] != actual_screen[i : i + 3]
        for i in range(0, len(expected_screen), 3)
    )
    result = dict(
        start=start,
        slots=compared_slots,
        slot_differences=differences,
        first_slot_differences=first,
        ram_words_different=ram_differences,
        final_pixels_different=pixels,
    )
    stream = native / "screen-trace.rgb"
    if stream.exists():
        expected = stream.read_bytes()
        observed = (emulator / "screen-trace.rgb").read_bytes()
        size = 96 * 64 * 3
        frames = len(expected) // size - start
        if len(observed) < frames * size:
            raise ValueError("Emulator framebuffer stream is too short")
        bad_frames = [
            f
            for f in range(frames)
            if expected[(start + f) * size : (start + f + 1) * size]
            != observed[f * size : (f + 1) * size]
        ]
        result["display_frames_different"] = len(bad_frames)
        result["first_display_difference"] = bad_frames[0] if bad_frames else None
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("native", type=Path)
    parser.add_argument("emulator", type=Path)
    parser.add_argument("--require-match", action="store_true")
    args = parser.parse_args()
    result = compare(args.native, args.emulator)
    print(json.dumps(result, indent=2))
    if args.require_match:
        # This is a native-circuit comparison, including its deterministic
        # carry/overflow outputs where the abstract ISA leaves them unspecified.
        failures = list(result["slot_differences"].values())
        failures += [
            result["ram_words_different"],
            result["final_pixels_different"],
            result.get("display_frames_different", 0),
        ]
        if any(failures):
            raise SystemExit(1)


if __name__ == "__main__":
    main()
