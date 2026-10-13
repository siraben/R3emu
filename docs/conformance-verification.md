# Conformance repair verification

Verification completed locally on 2026-10-13, before the subsequent request to
commit and publish the repairs. The immutable pre-fix audit remains under `/Users/siraben/doc/r3-conformance-audit/`; new raw
native captures, comparisons and build logs are under
`/Users/siraben/doc/r3-conformance-fixes/`.

## Requirement-to-evidence audit

| Finding | Implementation | Evidence |
|---|---|---|
| F1: memory service slots | `vm::Pipeline`, incoming transactions and retained prefetch | Native load/store, self-modification and frame-boundary fixtures; `tests/cycle_timing.rs` |
| F2: external waits and routing | `Bus::transact`, `TerminalPort`, optional conventional termination | Native unserved reads/writes and all per-unit bus requests/waits; bare-mirror and CLI tests |
| F3: S-unit eligibility | Exact operand encodings, previous-M destination conflicts, frame feedback | Six native fusion edge cases, original alias case and VM unit tests |
| F4: programmable glyph | Left/right register banks and measured bit order/polarity | Native 64-bit walking-clear fixture and register-reprogramming race |
| F5: complete font | Pinned 256-entry R316 ROM | Three native character-bank fixtures, including glyph zero |
| F6: scroll mask | Primary-axis bitmap replaces the range selection | Native masked-scroll fixture and independent small-grid tests |
| F7: directed endpoints | Ordered endpoints and five-bit directional cursor arithmetic | Native reverse-scroll/cursor fixtures; small-grid tests |
| F8: column orientation | Both source and destination coordinates transformed | Native column-scroll/reverse-cursor fixtures; small-grid tests |
| F9: peripheral timing | Controller/render separation, +2-frame visibility, retry backpressure and halted-CPU draining | Every display frame in 36 native timelines, busy-write/busy-read cases, halted-pipeline unit test |
| F10: configuration | `MachineConfig`, `--profile showcase`, explicit layouts/connector and bare-bus options | Profile and CLI tests; native fixtures use the same library configuration |
| F11: EXH disassembly | Distinct `exhs` and `exh` mnemonics | Exact encoded-word regression test |
| Readability / formatting | Typed pipeline/request/response states, independent configuration, immutable render operations, public observer API | rustfmt, strict all-target Clippy, Black and StyLua |

Additional native discrepancies found during repair were resolved: JMP's link
upper half, the keyboard's full-word sentinel, and native carry/overflow outputs
where the ISA leaves them unspecified. Cross-frame M-to-S feedback is supported
and unit-tested, with the feedback wiring documented in the pinned generator.

## Acceptance results

- **51 Rust tests passed**, including the aggregate native-conformance test.
- **38 native captures**: exact PC/state/flag/request/wait agreement across
  **68,750 slots**, exact **5,938 display frames**, and exact final RAM/screens.
- **320 independent wide-integer checks**: 128 arithmetic results and 192 flags,
  including both carry inputs for ADC/SBB.
- **8,704** independent small-grid scrolling cases cover direction, orientation,
  rectangles and every four-bit mask.
- `cargo test --all-targets --locked` passed, including CLI and interactive ROMs.
- `cargo clippy --all-targets --locked -- -D warnings` passed.
- `cargo fmt --all -- --check` passed.
- Release build and `cargo bench --bench bee_lzss --locked -- --test` passed.
- Black/StyLua formatting and Python syntax compilation passed.
- Nix package build and package tests passed. The packaged Bee ROM smoke halted
  at IP 6961 after 95,519 simulated frames.
- An additional release/stdout smoke produced 10,850 bytes, including the known
  opening text, and halted. SHA-256:
  `02cfc9a5dd3da30c9704788868f1a85b64a93e3bf40387e076997e1dc5afa2fe`.
- A fresh end-to-end native capture using the final formatted tools and the
  production observer passed `compare.py --require-match`.
- `git diff --check` passed. Existing unrelated untracked files were preserved.

The old interactive-ROM regression assumed that a key read and its conditional
branch completed in one VM frame. It now uses a bounded check respecting service
on the last execution unit; native slot tests enforce precise timing separately.
No failing conformance assertion was removed to make the suite pass.

## Build identity and retained logs

Nix builds from Git paths omit untracked source files. To test the complete local
change without staging unrelated work, the package build used an isolated
snapshot containing current tracked files plus this task's new source/tests/tools.
`nix-source-hashes.json` records those files. All compiled source, manifests,
fixtures and tools were hash-compared with the final working tree; only progress
and verification documentation changed after the snapshot.

Durable logs are in `/Users/siraben/doc/r3-conformance-fixes/verification/`:

- `bg_xuf_mv68dihp_33.log`: full Rust tests, lint, release and benchmark smoke.
- `bg_xuf_mv68ddyu_32.log`: helper formatting and syntax checks.
- `bg_xuf_mv68hpk4_34.log`: Nix package build/tests and packaged executable smoke.
- `bg_xuf_mv68scgy_35.log`: final native/production end-to-end comparison.
- `bee-run.log`, `bee-output.txt`: release execution sanity check.

## Architecture notes

A load first issues its data read, then consumes a dedicated service slot. A
store first fetches the following instruction, then issues its write; that
prefetched instruction remains intact through bus retries and may execute when
the write is acknowledged. This preserves native self-modifying-code behavior
without collapsing the service slot.

The terminal controller updates control state and emits immutable render work.
Display work applies two frames later; automatic scrolling schedules a second
controller step and rejects writes for its retry frame. Keyboard reads have a
separate responder. VM cycles advance peripherals even when CPU state is halted.
The observer sees the production state machine directly rather than a copied
execution loop.

## Scope

See `tests/fixtures/r316/README.md` for pinned source hashes, capture/reproduction
commands, fixture format and limitations. This verifies the audited R316 v2 CPU
and terminal configuration; it is not a claim of exhaustive equivalence for
all saves, physical key matrices, external adapters or desktop UI events.
