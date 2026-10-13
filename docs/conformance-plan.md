# Cycle and peripheral conformance work

Goal: resolve every finding in the local R3 save conformance audit, with readable,
idiomatic Rust and native differential evidence. Publication was authorized
separately after implementation and verification completed.

Reference: R316 v2 `dbfb2b9446d58cfe81424774491fe2ef1c04f121`; native CPS 3236906,
16 RAM rows, execution units `msfffmsfff`, terminal on unit 9. Original audit and
immutable baseline evidence: `/Users/siraben/doc/r3-conformance-audit/`.

## Coordinator ledger

- [x] F3 / F11: S-core fusion eligibility and flag-preserving EXH disassembly;
      exact-encoding regression tests (not merely equal operand values).
- [x] F4--F8: full saved character ROM, programmable glyph zero, directed and
      column-oriented scrolling, mask semantics; exhaustive small-grid tests.
- [x] F1 / F2: explicit fetch/memory/service/wait slot state machine, per-unit
      bus routing and termination, correct self-modifying-code fetch ordering.
- [x] F9: terminal command pipeline, delayed display work, automatic scroll and
      busy backpressure, advancing peripherals even while the CPU is halted.
- [x] F10: explicit machine profile and core layout options, documented defaults
      and hardware-faithful showcase initial conditions.
- [x] Reproducible checked-in native fixtures and a public observation API;
      compare PCs/states per slot and framebuffer bytes per frame, not only halt
      results. Extend native probes to fusion edge cases, bus retries, pipeline
      boundaries, terminal busy handling and cursor/range edge cases.
- [x] Run existing ROM/CLI tests, new differential tests, formatting, strict
      Clippy, release build and benchmark smoke. Update README and coverage /
      limitations; audit every requirement before marking the goal complete.

## Design constraints

- Keep immutable audit baselines; put new native runs in separate directories.
- Preserve untracked `r3manual.md`, `tests/r3`, and unrelated TPT checkout edits.
- A VM cycle is one native frame comprising all configured execution-unit slots.
  Peripheral latency is in frames, not executed-instruction counts.
- Bare memory mirrors and conventionally terminated peripheral buses are distinct
  configurations. Unsupported external accesses must not silently become RAM.
- Match S-core *encoded* operands and previous-M destination conflicts, including
  equal-valued but differently encoded operands and immediate/register modes.
- Keep reference calculations independent of the implementation under test.
- Do not claim exhaustive peripheral coverage for unimplemented external devices.

## Progress

Initial inspection complete: production has deferred memory work but no service
slots, a globally connected immediate terminal, and value-based S-core matching.
The hardware generator confirms that S eligibility compares instruction bits,
not register values. Upstream terminal register semantics and bus termination
rules have been reviewed.

F3/F11: unit regressions passed, and six expanded native fusion edge cases match
all PCs, states, requests, waits, RAM and final pixels. EXH preserves its flag
update distinction. The full-suite acceptance results below supersede this
initial focused gate.

F4--F8: all original glyph/scroll probes and three native full-font banks match
final pixels exactly. Directed scrolling passes 8,704 independent snapshot-based
small-grid/mask/orientation comparisons. The 64-bit custom-glyph walking probe
also matches exactly. Native measurement exposed a manual error: 0x9FC0/1 hold
left/right halves in column order 0,2,1,3, not interleaved even/odd columns. The
implementation and register names now follow the actual R316 v2 circuit.

F1/F2: explicit incoming bus transactions, load/store service slots, retained
prefetch and unit-local wait handling are implemented. The first production
comparison matches every PC/state/request/wait slot of the full showcase and all
original probes. Only the new terminal-busy probe still differed in that first
pass, before the timed terminal implementation. Artifact:
`/Users/siraben/doc/r3-conformance-fixes/pipeline-v1-summary.json`.

F9: controller/render separation, glyph/scroll and pixel latency (+2 frames),
automatic-scroll retry and write backpressure are implemented and verified. The
expanded overlap probe corrected the provisional +1-frame pixel assumption to
+2. Glyph reprogramming, busy writes and independent keyboard reads all match the
native circuit. Halted CPUs continue advancing pending display work.

Additional findings: native flags use C/O/Z/S bit order (observer now translates
the internal Z/S/C/O order). The showcase's one RAM difference was JMP's missing
upper-half preservation. The hardware ALU preserves the primary register's upper
half even for the encoded jump condition; fixed and confirmed by a native
non-default-upper probe. Empty keyboard reads retain the native 0x10000000
sentinel. ISA-unspecified carry/overflow outputs now reproduce the physical
parallel adder, allowing strict comparison of every native flag bit.

F10: validated generic/showcase machine profiles, explicit M/S/F layouts,
connector selection, bare-bus mode and frame limits implemented. CLI tests verify
configuration, initial registers, conflict/error handling, wait-vs-mirror behavior
and destructive keyboard consumption. ROM byte order is explicitly little-endian.

Latest completed gate: all-target Rust tests, strict Clippy, release build,
Criterion benchmark smoke and rustfmt passed. All 38 native fixtures pass strict
comparisons over 68,750 slots and 5,938 display frames; final RAM/screens and all
320 independent arithmetic/flag references pass. Tools formatted with Black and
StyLua and Python syntax checked. The older interactive-ROM test's one-frame
input assumption was replaced with a bounded check respecting last-unit service.

Nix package build/tests and packaged-binary smoke passed from an isolated
snapshot containing current tracked and task-created files, without staging
unrelated work. Compiled source, manifests, fixtures and tooling match the
snapshot hashes. Final end-to-end native capture/production comparison passed.
Verification logs are retained; `conformance-verification.md` maps every finding
to implementation and evidence. All milestones are complete within the explicitly
documented conformance scope.
Immutable audit captures remain untouched; new evidence is isolated under
`/Users/siraben/doc/r3-conformance-fixes/`.
