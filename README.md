# R3emu

Emulator for LBPHacker's R3 computer in Rust. R3 is a family of computers built
inside **The Powder Toy**, with quasi-32-bit storage and a 16-bit ALU.

Originally written by Justus Wolff (technik_hea), with help from LBPHacker;
rewritten in Rust by siraben.

## Features

- Explicit execution-unit slots, memory service cycles, prefetch, bus retries, and RAM mirrors.
- Timed terminal rendering, automatic-scroll backpressure, programmable glyphs and directed/masked scrolling.
- SDL2 display, 16-color pixel plotting, keyboard input, and RAM visualization.
- Headless execution, terminal output to stdout, and scripted keyboard input.
- Memory/disassembly dumps, bounded instruction traces, and a Criterion benchmark.
- A reusable Rust library exposing the VM, bus, ALU, and peripherals.

One VM cycle is one simulated Powder Toy frame. Its execution units run
sequentially as one computer, not as host threads. The generic CLI profile uses
64 RAM rows and ten M units. `--profile showcase` selects save **3236906**:
16 rows, `msfffmsfff`, saved initial registers, and a terminal on unit 9.

The CPU and terminal are checked against actual headless native TPT observations,
including every execution-unit slot and display frame, not just final images.
See [native fixtures and coverage](tests/fixtures/r316/README.md) and the
[repair verification](docs/conformance-verification.md). This is a bounded
R316 v2 conformance target, not a simulation of arbitrary TPT particles, every
save revision, or unimplemented peripherals such as the R216/breakout adapters.

## Build

### Nix

```sh
nix build
./result/bin/r3emu --help
```

For development:

```sh
nix develop
cargo build --release --locked
```

### Cargo

Install Rust and the SDL2 development libraries, then run:

```sh
cargo build --release --locked
./target/release/r3emu --help
```

SDL2 is a build/link dependency even when running headless. On macOS it can be
installed with `brew install sdl2`; on Debian/Ubuntu, with
`sudo apt install libsdl2-dev`.

## Run

```sh
# Graphical terminal and RAM view
r3emu tests/demo.bin

# Run at full speed without a window; ROM output on stdout, diagnostics on stderr
r3emu tests/bee_lzss.bin --headless --stdout

# Supply keyboard bytes after one VM cycle (the default delay)
r3emu tests/wordle.bin --keyboard-input-file guesses.txt

# Run RAM exported from save 3236906 with its actual configuration
r3emu showcase.bin --profile showcase --headless --max-cycles 500

# Explicit spatial layout and peripheral connector
r3emu program.bin --memrows 16 --core-types msfffmsfff --terminal-unit 9
```

Use `./result/bin/r3emu` or `./target/release/r3emu` if the executable is not on
`PATH`. After HLT, pending display stages finish before the window closes or
headless execution returns. Programs waiting for input or an unserved bus address
keep running unless `--max-cycles` bounds the run; reaching this bound does not
pretend that the CPU halted.

ROMs contain **little-endian** 32-bit words. Empty, partial-word, and oversized
inputs are rejected. The CLI initializes unused RAM to R3 logical zero
(`0x20000000`); loaded words are sanitized according to R3 write semantics.
The bundled `tests/demo.bin` is not the RAM image of showcase save 3236906.

Scripted input is consumed one byte per keyboard read. Literal `--keyboard-input`
bytes precede bytes from `--keyboard-input-file` when both are supplied. The
entire queue becomes available after the selected number of VM cycles (one
cycle runs all configured execution units), not with a delay between keys.
Live keypresses take priority. Graphical keyboard input supports lowercase
letters, digits, Return, Backspace, Tab, Space, and Escape.

### Options

Run `r3emu --help` for the authoritative list.

| Option | Default | Meaning |
| --- | --- | --- |
| `--profile` | generic | `generic` or `showcase`; explicit options override profile defaults |
| `--memrows` | profile | RAM rows; 64 generic, 16 showcase; nonzero |
| `--rowsize` | 128 | Words per RAM row; nonzero; total RAM must fit in 65,535 words |
| `--cores` | profile | Override with this many M units; conflicts with `--core-types` |
| `--core-types` | profile | Spatial M/S/F layout, e.g. `msfffmsfff` |
| `--terminal-unit` | last | Zero-based terminal connector unit |
| `--bare-bus` | off | Disconnect peripherals/terminators and expose read-only RAM mirrors |
| `--max-cycles` | unlimited | Bound all simulated frames, including display draining |
| `--targetfps` | 60 | Graphical VM cycles per second; 0 disables limiting |
| `--no-fpslimiter` | off | Disable graphical rate limiting |
| `--updxframes` | 10 | VM cycles between redraws (0 also redraws every cycle) |
| `--term-cols` / `--term-rows` | 12 / 8 | Terminal size in 8×8 cells; each dimension 1–29 |
| `--no-pixplot` | off | Disable pixel-plot I/O |
| `--no-smul` | off | Diagnostic override disabling S-type multiplication |
| `--headless` | off | No SDL window or rate limiting |
| `--stdout` | off | Mirror printed terminal characters to stdout |
| `--keyboard-input` | unset | Literal keyboard input bytes |
| `--keyboard-input-file` | unset | Read keyboard input bytes from a file |
| `--keyboard-input-delay-cycles` | 1 | VM cycles before scripted input is available |
| `--memdump` | off | Write memory and disassembly after emulation |
| `--tracedump` | off | Collect and write an instruction trace |
| `--tracesize` | 100000 | Maximum trace entries; retains the first entries, not a ring buffer |

The Haskell ROMs `bignum_pow2.bin` and `triangle.bin` wait for a key before
starting evaluation and do not automatically halt. For example:

```sh
r3emu tests/bignum_pow2.bin --headless --stdout --keyboard-input x
```

This prints `2^64 = 18446744073709551616` and waits for more input; use Ctrl-C
to stop it. The triangle program takes substantially longer to evaluate.

The standard R3 memory layout uses 128 words per row and up to 64 rows.
Other RAM dimensions are emulator extensions, not standard hardware configurations.
Conventional buses inject wait cycles for external accesses not handled on the
current unit. Bare-bus mirrors are a separate configuration, not a fallback for
missing devices. The terminal occupies `0x9F80..0x9FFF`; writes during an automatic
scroll retry wait, while its separate keyboard reader remains available.

For the pinned hardware, ordinary glyph/scroll and pixel changes become visible
two frames after acknowledgement. Automatic scrolling adds a retry frame.
The native circuit packs custom character registers as left/right halves in
column order 0,2,1,3, despite the reference manual's even/odd names. The emulator
follows the measured circuit. Carry/overflow values left unspecified by the ISA
also follow the native parallel adder; programs should not rely on them for
portability across R3 variants.

Dumps overwrite `memdump.bin`, `memdumpdisasm.asm`, `tracedump.bin`, and
`tracedumpdisasm.asm` in the working directory as applicable. Despite their
`.bin` suffix, memory and trace dumps are **decimal text**, not reloadable ROMs.
Dump write errors are currently not reported reliably.

## Example ROMs and assembler

Prebuilt ROMs are in `tests/`:

| ROM | Description |
| --- | --- |
| `bee_lzss.bin` | Bee Movie script decompression (LZSS) |
| `bignum_pow2.bin` | Haskell program calculating 2^64 |
| `triangle.bin` | Haskell program drawing a Sierpinski triangle |
| `demo.bin` | Original R3 demo |
| `colortest.bin` | Display color test |
| `demodism.bin` | Reassembled disassembly of the demo |
| `pixplot.bin` | Sierpinski carpet |
| `test0.bin` | Early emulator test |
| `test_fixes.bin` | Arithmetic/instruction regression ROM |
| `wordle.bin` | entropite's Wordle, built with the bundled C compiler |

The standalone TPTASM copy requires Lua. Supply the model explicitly outside
The Powder Toy:

```sh
lua tests/tptasm.lua tests/demo.asm /tmp/demo.bin model=R3A0564
# Rebuild the six assembly examples listed in tests/buildtests.lua:
make buildtests
```

The compiler in [`tests/compiler/`](tests/compiler/README.md) emits R3 assembly
and additionally requires LPEG. The three Haskell-generated ROMs do not have
source programs in this repository.

## Development

Inside `nix develop`, or with Rust and SDL2 installed:

```sh
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo fmt --check
cargo bench --bench bee_lzss --locked
```

Tests include native slot/frame fixtures, 128 independently checked arithmetic
results, 192 add/sub/ADC/SBB flag checks, every ROM glyph, all 64 custom-glyph bits,
bus and pipeline boundary cases, and 8,704 small-grid scrolling comparisons. They also
cover CLI validation, batch ROM execution and interactive ROM startup. Native
fixtures run offline; the capture/export tools are under `tools/conformance/`.

`MachineConfig` provides validated profiles; `Vm::cycle_with_observer` exposes
post-slot state without duplicating execution logic. The
`conformance_trace` example records the production VM for differential checks.
Direct bus `read`/`write` and terminal drawing methods are untimed inspection or
rendering primitives; use VM cycles for hardware timing.

The Criterion benchmark runs `bee_lzss.bin` headlessly, excluding VM setup from
the timed routine. This is not exhaustive ISA/state-space validation. Native
empty-keyboard and busy-read behavior is covered; SDL event translation, every
showcase demo selection and arbitrary external devices require separate validation.

`make compile`, `make test`, and `make clean` wrap Cargo. The former C/C++ Meson
build is no longer used. Existing third-party notices are retained in
[`src/LICENSE`](src/LICENSE) and [`tests/compiler/LICENSE`](tests/compiler/LICENSE).
