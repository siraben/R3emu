#!/usr/bin/env python3
"""Export compact golden data from native captures ONLY, never emulator output.

Consecutive identical slots are run-length encoded; screen changes are sparse.
See tests/fixtures/r316/README.md for the independent capture/rebuild procedure.
"""

import argparse
import csv
import hashlib
import json
from pathlib import Path
import struct

PALETTE = [
    tuple((level if bit else base) for bit in (i & 4, i & 2, i & 1))
    for i in range(16)
    for base, level in [(85 if i & 8 else 0, 255 if i & 8 else 170)]
]


def words(path):
    data = path.read_bytes()
    return list(struct.unpack("<" + "I" * (len(data) // 4), data))


def pixels(data):
    lookup = {rgb: index for index, rgb in enumerate(PALETTE)}
    return [lookup[tuple(data[i : i + 3])] for i in range(0, len(data), 3)]


def export(native, name):
    if not (native / "DONE").exists() or (native / "ERROR").exists():
        raise ValueError("Capture did not complete successfully")
    metadata = dict(
        line.split("=", 1)
        for line in (native / "metadata.txt").read_text().splitlines()
    )
    if metadata["model"] != "R3A1610" or metadata["core_types"] != "msfffmsfff":
        raise ValueError("Unexpected native machine configuration")
    with (native / "trace.csv").open() as stream:
        trace = list(csv.DictReader(stream))
    start = next(int(row["frame"]) for row in trace if int(row["state"]) & 15 == 1)
    frames = int((native / "DONE").read_text()) - start
    slots = []
    count = 0
    for row in trace:
        if int(row["frame"]) <= start:
            continue
        frame, core = divmod(count, 10)
        if (int(row["frame"]) - start, int(row["core"])) != (frame + 1, core):
            raise ValueError("Missing or unordered native slot")
        value = [
            int(row["pc"]) & 0xFFFF,
            int(row["state"]) & 15,
            int(row["flags"]) & 15,
            int(row["ram_addr"]) & 0xFFFFF,
            bool(int(row["io_state"]) & 1),
        ]
        if slots and slots[-1][1:] == value:
            slots[-1][0] += 1
        else:
            slots.append([1] + value)
        count += 1
    if count != frames * 10:
        raise ValueError("Wrong slot count")
    initial = words(native / "loaded.bin")
    final = words(native / "final.bin")
    final_pixels = pixels((native / "screen-final.ppm").read_bytes().split(b"\n", 3)[3])
    screens = []
    screen_path = native / "screen-trace.rgb"
    if screen_path.exists():
        data = screen_path.read_bytes()
        size = 96 * 64 * 3
        previous = [0] * (96 * 64)
        for frame in range(frames + 1):
            current = pixels(data[(frame + start) * size : (frame + start + 1) * size])
            changes = [
                [i, value] for i, value in enumerate(current) if value != previous[i]
            ]
            if changes:
                screens.append([frame, changes])
            previous = current
        if previous != final_pixels:
            raise ValueError("Final framebuffer differs from the recorded stream")
    artifacts = ["loaded.bin", "final.bin", "trace.csv", "screen-final.ppm"]
    if screen_path.exists():
        artifacts.append("screen-trace.rgb")
    return dict(
        name=name,
        native_start_frame=start,
        frames=frames,
        source_sha256={
            file: hashlib.sha256((native / file).read_bytes()).hexdigest()
            for file in artifacts
        },
        registers=[0x20000000] + [int(trace[9][f"r{i}"]) for i in range(1, 32)],
        initial_ram=[[i, word] for i, word in enumerate(initial) if word != 0x20000000],
        final_ram_changes=[
            [i, word] for i, word in enumerate(final) if word != initial[i]
        ],
        slots=slots,
        has_screen_timeline=screen_path.exists(),
        screens=screens,
        final_pixels=[[i, color] for i, color in enumerate(final_pixels) if color],
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--native", type=Path, required=True)
    parser.add_argument("--name", required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    fixture = export(args.native, args.name)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(fixture, separators=(",", ":")) + "\n")
    print(args.out)


if __name__ == "__main__":
    main()
