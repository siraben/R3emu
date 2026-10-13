# R3emu

Emulator for LBPHacker's R3 computer in Rust. R3 is a family of computers built
inside **The Powder Toy**, with quasi-32-bit storage and a 16-bit ALU.

Originally written by Justus Wolff (technik_hea), with help from LBPHacker;
rewritten in Rust by siraben.

## Features

- R3 instruction execution, flags, delayed memory operations, and read-only RAM mirrors.
- SDL2 terminal, 16-color pixel plotting, keyboard input, and RAM visualization.
- Headless execution, terminal output to stdout, and scripted keyboard input.
- Memory/disassembly dumps, bounded instruction traces, and a Criterion benchmark.
- A reusable Rust library exposing the VM, bus, ALU, and peripherals.

The CLI uses ten multiply-capable (M-type) execution units by default. These
execute sequentially as one computer, not as host threads. The library also
exposes F- and S-type units. This is not a simulation of all Powder Toy hardware
or external peripheral timing.

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
```

Use `./result/bin/r3emu` or `./target/release/r3emu` if the executable is not on
`PATH`. The window closes when the program halts. Programs that wait for input
or loop forever will keep running; headless mode has no automatic timeout.

ROMs contain native-endian 32-bit words, matching the original emulator and
bundled binaries on little-endian hosts. Empty, partial-word, and oversized
inputs are rejected. Unloaded RAM starts physically zero; loaded words are
sanitized according to R3 write semantics.

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
| `--memrows` | 64 | RAM rows; nonzero |
| `--rowsize` | 128 | Words per RAM row; nonzero; total RAM must fit in 65,535 words |
| `--cores` | 10 | M-type execution units per VM cycle; nonzero |
| `--targetfps` | 60 | Graphical VM cycles per second; 0 disables limiting |
| `--no-fpslimiter` | off | Disable graphical rate limiting |
| `--updxframes` | 10 | VM cycles between redraws (0 also redraws every cycle) |
| `--term-cols` / `--term-rows` | 12 / 8 | Terminal size in 8×8 character cells; nonzero |
| `--no-pixplot` | off | Disable pixel-plot I/O |
| `--no-smul` | off | Disable S-type multiplication (no effect on the CLI's M-only configuration) |
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
Other dimensions are emulator extensions, not standard hardware configurations.

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
cargo clippy --all-targets --locked
cargo fmt --check
cargo bench --bench bee_lzss --locked
```

Tests cover arithmetic, memory mapping, instruction semantics, keyboard queues,
CLI validation, bounded execution of batch ROMs, and interactive ROM startup. The benchmark runs
`bee_lzss.bin` headlessly, excluding VM setup from the timed routine. These tests
are not a complete hardware-conformance suite; graphical rendering and external
hardware timing need separate validation.

`make compile`, `make test`, and `make clean` wrap Cargo. The former C/C++ Meson
build is no longer used. Existing third-party notices are retained in
[`src/LICENSE`](src/LICENSE) and [`tests/compiler/LICENSE`](tests/compiler/LICENSE).
