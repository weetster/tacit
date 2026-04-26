# Tacit — Development Guide

Tacit is an AI-first programming language. See [plans/tacit-plan.md](plans/tacit-plan.md) for the full vision and [plans/phase-1-plan.md](plans/phase-1-plan.md) for current work.

**Current phase: Phase 1 — minimum viable compiler (complete).** Phase 0 is frozen ([ADR 0013](decisions/0013-canonical-text-format-frozen.md), [ADR 0017](decisions/0017-stage-3-frozen.md), [ADR 0018](decisions/0018-stage-5-frozen.md)). Phase 1 Stages 1–5 are all complete; the phase is frozen pending a Phase 1 freeze ADR before Phase 2 begins.

## What Phase 1 produces

- A working pipeline: `.tac` (authoring view) → AST → LLVM IR → object file → executable.
- Lossless round-trip between authoring view and canonical text via sidecar (`tacit-views::authoring`).
- Inspection-view renderer (display-only, L0/L1/L2 layers) per [ADR 0015](decisions/0015-inspection-view-scope.md).
- LLVM IR emitter for the Phase 1 codegen subset (closed lambdas, `rec`, `if`, `let`, `match`, `@name` primitives) — see [docs/compiler-architecture.md § Phase 1 codegen subset](docs/compiler-architecture.md).
- A frozen seven-program smoke corpus under `examples/smoke/` exercising the emitter end-to-end.
- `tacit` CLI exposing `compile` and `view` subcommands (Stage 5, complete).

Phase 1 exit is gated on: end-to-end CI runs each smoke program, the authoring round-trip property holds on every Phase 0 test vector in scope, and inspection-view § 6 fixtures render byte-identically.

## Ground rules for this phase

- **Frozen artifacts stay frozen.** The canonical text format ([ADR 0013](decisions/0013-canonical-text-format-frozen.md)), Stage 3 view grammars + AST enum ([ADR 0017](decisions/0017-stage-3-frozen.md)), Stage 5 repo scaffolding ([ADR 0018](decisions/0018-stage-5-frozen.md)), and Phase 1 Stages 1–4 ([ADR 0032](decisions/0032-stage-4-frozen.md)) are all frozen. Changes require a new ADR and are treated as spec bugs, not scope negotiation.
- **Spec ambiguities found in Phase 1 are bugs against Phase 0.** Resolve via a new ADR against the relevant frozen artifact, not in-line spec edits. ADRs 0030 and 0031 are examples — they extended the `@name` allowlist and locked the LLVM distribution model after Stage 4 surfaced the gaps.
- **Two views from day one.** Authoring and inspection grammars exist together. Don't let either rot — round-trip and rendering tests are the load-bearing checks.
- **Decision log is load-bearing.** Every non-trivial design choice gets an ADR-style entry in `decisions/NNNN-title.md`. This is how the spec stays coherent across sessions.

## Key design commitments (do not relitigate)

- Variable references use **DeBruijn indices** in canonical text; no variable IDs. Names are display metadata only.
- Mutual recursion uses explicit `rec { ... }` groupings that hash as a single atom.
- Parser errors produce **typed `Hole` nodes** with structured diagnostics, not failed parses. Phase 1 codegen hard-fails on holes ([ADR 0023](decisions/0023-hole-node-recovery-deferred.md)); recovery is deferred to Phase 2.
- **BLAKE3** is the hash.
- Display names, comments, and file layout are all sidecar / advisory. The AST is the source of truth.
- Tacit-Lite is the default focus. Tacit-Full features (refinement types, capabilities, handlers) are out of scope for Phase 1–6.
- LLVM 19 is pinned via `inkwell` 0.9's `llvm19-1` feature ([ADR 0032 § 1](decisions/0032-stage-4-frozen.md)). Bumping is a deliberate release-engineering task ([ADR 0031](decisions/0031-llvm-distribution-and-self-hosting.md)).

## Repository layout

```
plans/        — phase plans (tacit-plan.md, phase-0-plan.md, phase-1-plan.md), specs (canonical-text-format.md, inspection-view.md, sidecar-format.md), test vectors
docs/         — design docs (compiler-architecture.md, effect-system.md)
decisions/    — ADR-style decision log (0001–0032)
crates/       — Cargo workspace: tacit-canonical, tacit-views, tacit-codegen
examples/     — Phase 1 smoke corpus under smoke/
corpus/       — Phase 3 evaluation corpus (60 tasks, sealed held-out subset)
stdlib/       — libc-effects.toml (dormant Phase 1 effect signatures)
```

CI lives at `.github/workflows/ci.yml` with two jobs: Python (`uv run pytest`) and Rust (`cargo fmt --check`, `cargo clippy --all-targets --features tacit-codegen/llvm19-1 -- -D warnings`, `cargo test --features tacit-codegen/llvm19-1`).

## Open questions

All Phase 0 open questions and Phase 1 open questions Q-P1-1 through Q-P1-5 are resolved (see [phase-1-plan.md § Open questions](plans/phase-1-plan.md)). Phase 2 questions surface as the type-and-effect work begins.

## Working style

- Prefer editing existing plan/spec files over creating new ones.
- When a design choice is made, write the ADR before writing the spec text or code that depends on it.
- Phase 1 is implementation, not scaffolding. Add code where it lands cleanly; resist over-abstracting for hypothetical Phase 2 needs.
