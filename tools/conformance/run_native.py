#!/usr/bin/env python3
"""Capture save 3236906 in actual TPT, without desktop input automation.

The Lua observer uses this save's anchored CPU layout and fixed display position.
Supply an independently built native engine; this tool never substitutes a model.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

EXPECTED_SAVE_SHA256 = (
    "f906d99110020210a6108fad172647fcbc5e5c59feada9e53ff7bdb164eadde2"
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--engine", type=Path, required=True)
    parser.add_argument("--save", type=Path, required=True)
    parser.add_argument("--rom", type=Path)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--frames", type=int, default=200)
    parser.add_argument("--screen-trace", action="store_true")
    args = parser.parse_args()
    if args.frames <= 10:
        parser.error("--frames must exceed the requested startup frame (10)")
    engine = args.engine.resolve(strict=True)
    save = args.save.resolve(strict=True)
    rom = args.rom.resolve(strict=True) if args.rom else None
    save_hash = hashlib.sha256(save.read_bytes()).hexdigest()
    if save_hash != EXPECTED_SAVE_SHA256:
        parser.error(
            "save differs from the pinned CPS 3236906; review the observer before updating"
        )
    out = args.out.resolve()
    # Do not silently overwrite either successful or failed evidence.
    out.mkdir(parents=True, exist_ok=False)
    shutil.copyfile(Path(__file__).with_name("capture.lua"), out / "autorun.lua")
    config = dict(
        out=str(out),
        save=str(save),
        start=10,
        frames=args.frames,
        screen_trace=args.screen_trace,
    )
    if rom:
        config["rom"] = str(rom)
    entries = []
    for key, value in config.items():
        entries.append(f"{key}={json.dumps(value, ensure_ascii=False)}")
    entries.append(f"shots={{1,50,{args.frames}}}")
    (out / "capture-config.lua").write_text("return {" + ",".join(entries) + "}\n")
    provenance = {
        "engine": str(engine),
        "save": str(save),
        "save_sha256": save_hash,
        "rom": str(rom) if rom else None,
        "rom_sha256": hashlib.sha256(rom.read_bytes()).hexdigest() if rom else None,
        "frames": args.frames,
    }
    (out / "provenance.json").write_text(json.dumps(provenance, indent=2) + "\n")
    env = dict(os.environ, SDL_VIDEODRIVER="dummy", SDL_AUDIODRIVER="dummy")
    with (out / "native.log").open("w") as log:
        subprocess.run(
            [str(engine), "ddir", str(out), "disable-network", "disable-bluescreen"],
            cwd=out,
            env=env,
            stdout=log,
            stderr=subprocess.STDOUT,
            timeout=180,
            check=True,
        )
    if not (out / "DONE").exists() or (out / "ERROR").exists():
        raise RuntimeError(f"Incomplete native capture: inspect {out}")
    print(out)


if __name__ == "__main__":
    main()
