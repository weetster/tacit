# 0024 — Phase 1 LLVM bindings: `inkwell` from the start

**Status:** Accepted
**Date:** 2026-04-24
**Phase:** 1, Stage 4
**Closes:** [phase-1-plan.md Q-P1-5](../plans/phase-1-plan.md)

## Context

[phase-1-plan.md § Stage 4](../plans/phase-1-plan.md) is the critical-path
technical risk for Phase 1, and Q-P1-5 asks which LLVM binding layer the
new `tacit-codegen` crate should use. Three candidates were on the table:

- **Textual LLVM IR + `llc` subprocess.** The Rust compiler emits `.ll`
  text; `llc` is invoked as a subprocess to produce an object file; the
  system linker produces an executable. Zero LLVM link-time dependency in
  the Rust binary, IR is human-readable and greppable, golden-file tests
  are trivial. Costs: subprocess per compile, no programmatic IR
  manipulation, Phase 2's type/effect metadata has to ride as string
  concatenation, and the `tacit-codegen` crate's abstraction is string-
  shaped until a later rewrite replaces it with programmatic IR.
- **`inkwell`.** Idiomatic Rust wrapper over LLVM-C. Type-safe builders,
  in-process emission, the standard Rust LLVM choice. Costs: tightly
  version-coupled to a specific LLVM release (e.g., inkwell 0.5 → LLVM 18),
  large API surface, LLVM install becomes a contributor and CI
  prerequisite.
- **`llvm-sys`.** Raw unsafe FFI to LLVM-C. No advantage over `inkwell`
  at Phase 1's IR complexity, and commits the entire codegen crate to an
  unsafe surface. Worth keeping as an escape hatch when `inkwell` lacks
  a specific API, not worth starting on.

[tacit-plan.md § Backend](../plans/tacit-plan.md) names `inkwell`;
Q-P1-5 was left explicitly open to allow confirmation or override based
on a Stage 4 spike. The decision was surfaced early (before Stage 4 spike
work began) after weighing the rewrite-cost argument for textual IR
against inkwell's natural fit for Phase 2+ metadata.

Phase 2 will need IR that carries type and effect metadata in a form the
checker can read and manipulate. A textual-IR starting point commits the
codegen crate to an architecture where metadata is string concatenation
and reading metadata from existing IR means re-parsing text. Either the
codegen crate is rewritten when Phase 2 begins, or Phase 2 inherits a
string-shaped abstraction that resists its actual needs.

## Decision

**The `tacit-codegen` crate uses `inkwell` from the start of Stage 4.
Textual IR is available as an opt-in CLI dump for inspection, not as the
load-bearing representation.**

Concretely:

1. `tacit-codegen` depends on `inkwell`, with both the `inkwell` crate
   version and the target LLVM major version pinned in `Cargo.toml`
   and documented in `docs/compiler-architecture.md`. The exact version
   pair is chosen during Stage 4's familiarization spike; this ADR
   commits to pinning, not to a specific number.
2. IR is built programmatically via `inkwell`'s builders. Emission
   produces an LLVM `MemoryBuffer` written to an object file in-process;
   the system linker produces the executable.
3. `tacit compile` gains an `--emit-llvm-ir` flag that serialises the
   constructed `Module` to textual `.ll` output for debugging. Textual
   IR is an output, not an input — there is no round-trip through
   `.ll` at any point in the compile pipeline.
4. CI installs the pinned LLVM version via the runner's standard
   package mechanism; the ADR for the CI workflow (see
   [ADR 0018](0018-stage-5-frozen.md)) gains the install step as part
   of the Stage 4 landing.
5. `llvm-sys` remains available as an escape hatch. Using it from
   inside `tacit-codegen` (e.g., to call an LLVM API `inkwell` does not
   wrap) requires a follow-up ADR before the first such call lands.

## Alternatives considered

- **Textual IR + `llc` subprocess for Phase 1, switch to `inkwell` in
  Phase 2.** Rejected per the rewrite-cost argument. Phase 2's metadata
  work is the first time the codegen abstraction is under real pressure,
  and arriving at that moment with a string-based abstraction guarantees
  a rewrite. Paying the `inkwell` learning cost once is preferable to
  paying textual-IR plumbing plus the rewrite. The subprocess overhead
  (per-compile `llc` + linker spawn) is also a recurring cost that
  compounds across every smoke program and CI run.
- **`llvm-sys` directly.** Rejected. No advantage over `inkwell` for
  Phase 1's IR complexity, and the unsafe surface would dominate the
  codegen crate. Kept as an escape hatch only.
- **Cranelift instead of LLVM.** Rejected as out of scope.
  [tacit-plan.md § Backend](../plans/tacit-plan.md) commits to LLVM IR
  for the optimization story and for WASM-candidate parity
  (per [ADR 0022](0022-pure-kernel-host-model.md) § 3). Switching
  backends is a separate architectural decision with its own ADR; this
  ADR chooses the binding layer, not the backend.
- **Defer the choice until after a Stage 4 spike.** Considered and
  rejected. The spike's purpose was to confirm or override the
  parent-plan default; a confirmation ahead of the spike (grounded in
  the Phase 2 metadata argument) removes the risk that a
  textual-IR-shaped prototype gets built and then thrown away.
  The spike itself is still scheduled as a familiarization exercise
  (estimate 1–2 days), now scoped to "learn the `inkwell` surface we
  need," not "decide between three options."

## Consequences

- Stage 4 begins with a familiarization spike on `inkwell`. The schedule
  risk flagged in [phase-1-plan.md § Risks](../plans/phase-1-plan.md) is
  accepted; the rewrite-avoidance payoff is the justification.
- LLVM becomes a build-time dependency for every contributor and CI
  runner. Install instructions per platform land in
  `docs/compiler-architecture.md` as part of Stage 5.
- Phase 2's type/effect metadata extends naturally via `inkwell`'s
  metadata APIs; no migration is needed to carry richer IR.
- LLVM version upgrades become a tracked maintenance task. Each bump
  is a deliberate follow-up (check `inkwell` release notes, verify IR
  still compiles, run the smoke corpus). Not Phase 1 work, but
  scheduled explicitly rather than drifting.
- `--emit-llvm-ir` preserves IR inspectability without making textual IR
  load-bearing. Debugging a codegen bug still means reading `.ll`
  output; the codegen crate does not depend on parsing it back.
- The `llvm-sys` escape hatch is preserved but gated by ADR. Ad-hoc
  unsafe FFI does not creep into the codegen crate without a recorded
  decision.

## Related decisions

- [tacit-plan.md § Backend](../plans/tacit-plan.md) — names `inkwell`;
  this ADR confirms.
- [ADR 0018](0018-stage-5-frozen.md) — CI workflow; Stage 4 landing adds
  the LLVM install step.
- [ADR 0022](0022-pure-kernel-host-model.md) — logs WASM as a candidate
  backend for the host-model use case; does not affect Phase 1's
  native-LLVM choice.
- [phase-1-plan.md § Stage 4, § Open Questions Q-P1-5](../plans/phase-1-plan.md)
  — closed by this ADR.
- Future Phase 2+ ADR — any first use of `llvm-sys` inside
  `tacit-codegen` requires its own ADR.
