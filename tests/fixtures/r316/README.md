# Native R316 conformance fixtures

These are observations from **actual native Powder Toy**, not values generated
by the emulator. `cargo test --test native_conformance` runs offline and compares
38 captures: **68,750 execution-unit slots**, **5,938 display frames** across 36
captures, final RAM and final pixels. All four arithmetic flag bits are compared,
including native behavior where the abstract ISA leaves carry/overflow undefined.
Three reference tables under `../reference/` are checked using independent
wide-integer calculations: **128 arithmetic results and 192 flag results**,
including ADC/SBB with both carry inputs.

## Pinned source identity

- Save: <https://powdertoy.co.uk/GetSave.util?ID=3236906>, 44,053 bytes.
- CPS SHA-256: `f906d99110020210a6108fad172647fcbc5e5c59feada9e53ff7bdb164eadde2`.
- Native TPT: `2cfb22807c529525d7ef413d7c9651f5784747c1`.
- Hardware/reference: R316 v2 `dbfb2b9446d58cfe81424774491fe2ef1c04f121`,
  <https://github.com/LBPHacker/R316/tree/v2>.
- CPU anchor: `(203,241)`, machine-readable `R3A1610`, 16 x 128 RAM words,
  ten units `msfffmsfff`; terminal connected to unit 9.
- Terminal: 96 x 64 pixels at `(342,138)`, base address `0x9F80`.

The human-facing model name reverses the row/unit numbers. Decode the anchor;
do not infer geometry from the human label. The downloaded CPS and native source
were not modified. Lua flashes microprograms into the running simulation's RAM.
The native engine runs with SDL dummy drivers; no keyboard/mouse event automation
or simplified Python simulation is involved.

## Coverage

- Original audit: arithmetic, dependent load/store, self-modification, unmapped
  read, S multiply alias, ASCII/high/custom glyphs, four scrolling cases and
  automatic-scroll/no-scroll controls.
- Six additional S-fusion edge cases: same values in different registers,
  previous-input clobber with unchanged zero value, unused encoding bits,
  immediate/register mismatch, r0 alias and signed-high/low pairing.
- All 256 character indices and an independent walking-clear probe for all 64
  programmable glyph bits.
- Terminal busy writes, keyboard reads during a retry, empty-keyboard word,
  glyph-register changes during rendering, and reversed row/column cursors with
  nonblank newline glyphs and automatic scrolling.
- Unserved writes, load/store service crossing a frame boundary and JMP's link
  upper half.
- Full showcase boot/menu trace and three arithmetic/flag/carry reference matrices.

Native measurements expose some reference-manual inaccuracies. Custom character
registers at offsets 0x40/0x41 contain **left/right halves**, each in column order
0,2,1,3, rather than even/odd columns. Nonblank newline glyphs follow the circuit's
controller behavior. The code targets this pinned native design and documents
these differences rather than silently substituting the prose specification.

## Capture and reproduce

Build native TPT at the pinned revision and obtain the exact CPS above. From the
R3emu root, with `TPT` pointing to that native executable and `CPS` to the save:

```sh
python3 tools/conformance/generate_probes.py --out /tmp/r3-roms
python3 tools/conformance/run_native.py \
  --engine "$TPT" --save "$CPS" --rom /tmp/r3-roms/terminal-busy.bin \
  --out /tmp/r3-native-terminal-busy --frames 150 --screen-trace

cargo run --example conformance_trace -- \
  /tmp/r3-native-terminal-busy/loaded.bin /tmp/r3-rust-terminal-busy 150
python3 tools/conformance/compare.py \
  /tmp/r3-native-terminal-busy /tmp/r3-rust-terminal-busy --require-match

python3 tools/conformance/export_fixture.py \
  --native /tmp/r3-native-terminal-busy --name terminal-busy \
  --out /tmp/terminal-busy.json
```

The generator covers the expanded probes. Every fixture also contains the exact
loaded RAM image (sparse against `0x20000000`), allowing reproduction of the original
probes and showcase without depending on a different bundled demo binary:

```sh
python3 - <<'PY'
import json, struct
from pathlib import Path
f = json.loads(Path('tests/fixtures/r316/showcase.json').read_text())
ram = [0x20000000] * 2048
for address, value in f['initial_ram']:
    ram[address] = value
Path('/tmp/showcase.bin').write_bytes(struct.pack('<2048I', *ram))
PY
```

`run_native.py` checks the CPS hash and topology and refuses to overwrite an
existing output directory. The observer records simulation state in `AFTERSIM`,
starts the INST circuit through the API in `TICK`, and keeps screenshots separate
from logical pixel bytes. Startup propagation varies with interface scheduling;
comparisons align to the **observed** state transition, not the requested start
frame. Each fixture retains hashes of its native source artifacts.

## Compact fixture format

- `initial_ram`: non-default `(address, value)` pairs; default is `0x20000000`.
- `registers`: initial architectural register contents, including fixed r0.
- `slots`: run-length encoding of `(count, PC, state, flags, request, wait)`.
  State bits are Execute=1, Load=2, Store=4, Halted=8. Flags are C/O/Z/S.
  Request packs the 16-bit address plus native read/write/internal/external bits.
- `screens`: `(relative_frame, [(pixel_index, palette_index), ...])` changes,
  applied to an initially black 96 x 64 screen. Missing entries mean no changes.
- `final_ram_changes`: updates relative to the initial RAM, not a partial check.
- `final_pixels`: nonblack final pixels; all remaining pixels must be black.

The exporter reads **only native artifacts**. Never regenerate a golden fixture
from a Rust trace to make a regression pass. The native register rows circulate
through the physical pipeline and are not atomic post-instruction snapshots;
slot assertions use stable PC/state/flags/bus probes instead.

## Limits

This is bounded conformance evidence, not exhaustive equivalence. It does not
cover every operand/state combination, all showcase menu selections, a physical
keyboard's complete key matrix, interactive reset/halt-button timing, SDL event
translation, other CPS revisions, or R216/FILT/INST peripheral adapters. Keyboard
native probes here cover empty reads and reads during terminal busy frames;
nonempty scripted/live input consumption also has Rust/CLI regression tests.
Raw native CSV/RGB/particle captures and the original audit remain outside the
repository under `/Users/siraben/doc/r3-conformance-audit/` and
`/Users/siraben/doc/r3-conformance-fixes/`.
