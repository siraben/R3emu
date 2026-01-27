# R3emu
A (full) R3 emulator...
Why did I start making this again? Oh right cuz its 1 AM in the morning

Written by technik_hea, with help from LBPHacker who also made the original R3.
Rewritten in Rust by siraben.

# Building

## With Nix (recommended)

```
nix build
```

The emulator will be at `result/bin/r3emu`.

For a development shell with all dependencies:

```
nix develop
cargo build --release
```

## With Cargo

You need Rust and SDL2 development libraries installed.

```
cargo build --release
```

The emulator will be at `target/release/r3emu`.

# Usage

```
r3emu input.bin
```

How you can get these bin files? Simple: you assemble tptasm files. There is a tpt assembler shipped with the
repo under tests/tptasm.lua but for that you need to have Lua installed on your system.
Then you can call tptasm from any file and use it as usual to output a bin file.

There are also a few tpt assembly files under tests as well as their bin equivalents.
These include:

| ROM | Description |
| --- | --- |
| bee_lzss | Bee movie script decompression (LZSS) |
| bignum_pow2 | Haskell program calculating 2^64 |
| triangle | Haskell program drawing Sierpinski triangle |
| demo | the original demo running on the R3 |
| colortest | a testing ROM to test the display colors |
| demodism | same as demo, except a disassembled version from a disassembler |
| pixplot | Sierpinski carpet |
| test0 | a basic testing ROM made in the early hours of development |
| wordle | the wordle game from entropite with their C compiler (copy under /tests/compiler) |

NOTE: the emulator automatically closes the window as soon as the emulation finishes.

# Config

All configuration is done via CLI flags. Run `r3emu --help` for the full list.

| Flag | Default | Description |
| --- | --- | --- |
| `--memrows` | 64 | Amount of memory rows emulated |
| `--cores` | 10 | Number of cores to emulate (all multiply-capable) |
| `--targetfps` | 60 | Target FPS |
| `--no-fpslimiter` | off | Disable the FPS limiter |
| `--updxframes` | 10 | Frames between SDL window updates |
| `--memdump` | off | Dump memory and disassembly after emulation |
| `--tracedump` | off | Dump instruction trace after emulation |
| `--tracesize` | 100000 | Trace buffer size |
| `--term-cols` | 12 | Terminal character columns |
| `--term-rows` | 8 | Terminal character rows |
| `--rowsize` | 128 | Memory row size in words |
| `--no-pixplot` | off | Disable pixel plotting |
| `--no-smul` | off | Disallow S-type core multiplication |
| `--stdout` | off | Mirror terminal output to stdout |
| `--headless` | off | Run without UI (no SDL2 window) |

I think I explained everything now. For questions: technik_hea on DC

