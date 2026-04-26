# Tacit — Development Guide

Tacit is an AI-first programming language. See [plans/tacit-plan.md](plans/tacit-plan.md) for the full vision and [plans/phase-2-plan.md](plans/phase-2-plan.md) for current work (once created).

**Current phase: Phase 2 — type and effect system.** Phase 1 is frozen ([ADR 0033](decisions/0033-phase-1-frozen.md)); all five stages are complete. Phase 2 scope lives in [plans/phase-2-plan.md](plans/phase-2-plan.md), layered onto [plans/tacit-plan.md § Phase 2](plans/tacit-plan.md).

## What Phase 1 produced (frozen baseline)

- A working pipeline: `.tac` (authoring view) → AST → LLVM IR → object file → executable.
- Lossless round-trip between authoring view and canonical text via sidecar (`tacit-views::authoring`).
- Inspection-view renderer (display-only, L0/L1/L2 layers) per [ADR 0015](decisions/0015-inspection-view-scope.md).
- LLVM IR emitter for the Phase 1 codegen subset (closed lambdas, `rec`, `if`, `let`, `match`, `@name` primitives) — see [docs/compiler-architecture.md](docs/compiler-architecture.md).
- Seven-program smoke corpus under `examples/smoke/` exercising the emitter end-to-end.
- `tacit` CLI exposing `compile` and `view` subcommands.

## What Phase 2 will add

Per [tacit-plan.md § Phase 2](plans/tacit-plan.md):

- Local type inference within function bodies; explicit signatures at export boundaries.
- Structural type checking (no refinements).
- Basic generic types.
- Simple effect system: fixed `IO`/`Alloc`/`Mut`/`Div` lattice, local inference, mandatory annotations at module boundaries, basic effect polymorphism for higher-order functions.
- Effect signatures for the libc-wrapper stdlib (consuming `stdlib/libc-effects.toml`).
- Structured error reporting format (JSON-emittable) for type and effect errors.
- A new `tacit-typecheck` crate, wired into `tacit-cli`.

Phase 2 exit: non-trivial programs (sorting algorithms, basic data structures, file I/O) typecheck with correct effect annotations and compile.

## Ground rules

- **Frozen artifacts stay frozen.** The canonical text format ([ADR 0013](decisions/0013-canonical-text-format-frozen.md)), Stage 3 view grammars + AST enum ([ADR 0017](decisions/0017-stage-3-frozen.md)), Stage 5 repo scaffolding ([ADR 0018](decisions/0018-stage-5-frozen.md)), Phase 1 Stages 1–4 ([ADR 0032](decisions/0032-stage-4-frozen.md)), and Phase 1 as a whole ([ADR 0033](decisions/0033-phase-1-frozen.md)) are all frozen. Changes require a new ADR and are treated as spec bugs, not scope negotiation.
- **Spec ambiguities are bugs against the relevant frozen artifact.** Resolve via a new ADR, not in-line spec edits.
- **Two views from day one.** Authoring and inspection grammars exist together. Round-trip and rendering tests are the load-bearing checks.
- **Decision log is load-bearing.** Every non-trivial design choice gets an ADR-style entry in `decisions/NNNN-title.md`.

## Key design commitments (do not relitigate)

- Variable references use **DeBruijn indices** in canonical text; no variable IDs. Names are display metadata only.
- Mutual recursion uses explicit `rec { ... }` groupings that hash as a single atom.
- Parser errors produce **typed `Hole` nodes** with structured diagnostics, not failed parses. Recovery is deferred to Phase 2 ([ADR 0023](decisions/0023-hole-node-recovery-deferred.md)).
- **BLAKE3** is the hash.
- Display names, comments, and file layout are all sidecar / advisory. The AST is the source of truth.
- Tacit-Lite is the default focus. Tacit-Full features (refinement types, capabilities, handlers) are out of scope for Phases 1–6.
- LLVM 19 is pinned via `inkwell` 0.9's `llvm19-1` feature ([ADR 0032 § 1](decisions/0032-stage-4-frozen.md)). Bumping is a deliberate release-engineering task ([ADR 0031](decisions/0031-llvm-distribution-and-self-hosting.md)).

## Repository layout

```
plans/        — phase plans, specs (canonical-text-format.md, inspection-view.md, sidecar-format.md), test vectors
docs/         — design docs (compiler-architecture.md, effect-system.md)
decisions/    — ADR-style decision log (0001–0033)
crates/       — Cargo workspace: tacit-canonical, tacit-views, tacit-codegen, tacit-cli
examples/     — Phase 1 smoke corpus under smoke/
corpus/       — Phase 3 evaluation corpus (60 tasks, sealed held-out subset)
stdlib/       — libc-effects.toml (dormant Phase 1 effect signatures; Phase 2 consumes this)
```

CI lives at `.github/workflows/ci.yml`: Python (`uv run pytest`), Rust (`cargo fmt --check`, `cargo clippy --all-targets --features tacit-codegen/llvm19-1,tacit-cli/llvm19-1 -- -D warnings`, `cargo test --features tacit-codegen/llvm19-1,tacit-cli/llvm19-1`), and a CLI smoke step that builds and runs `tacit compile examples/smoke/hello.tac`.

## Open questions

All Phase 0 and Phase 1 open questions are resolved (see [phase-1-plan.md § Open questions](plans/phase-1-plan.md)). Phase 2 questions surface as the type-and-effect work begins; they will be enumerated in `phase-2-plan.md`.

## Working style

- Prefer editing existing plan/spec files over creating new ones.
- When a design choice is made, write the ADR before writing the spec text or code that depends on it.
- Phase 2 starts with `phase-2-plan.md` before any implementation. Resist over-abstracting beyond Phase 2's stated scope.
