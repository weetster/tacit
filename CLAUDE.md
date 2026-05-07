# Tacit — Development Guide

Tacit is an AI-first programming language. See [plans/tacit-plan.md](plans/tacit-plan.md) for the full vision and [plans/phase-3-plan.md](plans/phase-3-plan.md) for the just-frozen phase.

**Current phase: Phase 3 complete; Phase 4 is next.** Phase 3 is frozen ([ADR 0070](decisions/0070-p3-frozen.md)); the Python-relative density gate was retired as structurally miscalibrated and density work going forward tracks against Rust. Phase 2 is frozen ([ADR 0046](decisions/0046-p2-stage-5-frozen.md)). Phase 1 is frozen ([ADR 0033](decisions/0033-phase-1-frozen.md)). Phase 4 scope will live in `plans/phase-4-plan.md` (not yet created).

## What Phase 1 produced (frozen baseline)

- A working pipeline: `.tac` (authoring view) → AST → LLVM IR → object file → executable.
- Lossless round-trip between authoring view and canonical text via sidecar (`tacit-views::authoring`).
- Inspection-view renderer (display-only, L0/L1/L2 layers) per [ADR 0015](decisions/0015-inspection-view-scope.md).
- LLVM IR emitter for the Phase 1 codegen subset (closed lambdas, `rec`, `if`, `let`, `match`, `@name` primitives) — see [docs/compiler-architecture.md](docs/compiler-architecture.md).
- Seven-program smoke corpus under `examples/smoke/` exercising the emitter end-to-end.
- `tacit` CLI exposing `compile` and `view` subcommands.

## What Phase 2 produced (frozen baseline)

Per [ADR 0046](decisions/0046-p2-stage-5-frozen.md):

- Local type inference within function bodies; explicit signatures at export boundaries.
- Structural type checking (no refinements); basic generic types.
- Fixed-lattice effect system (`IO`/`Alloc`/`Mut`/`Div` per [ADR 0035](decisions/0035-p2-effect-set-canonical.md)) with local inference, mandatory annotations at module boundaries, and basic effect polymorphism for higher-order functions ([ADR 0036](decisions/0036-p2-effect-polymorphism-syntax.md)).
- Effect signatures for the libc-wrapper stdlib (consuming `stdlib/libc-effects.toml`).
- Structured error reporting format ([ADR 0041](decisions/0041-p2-structured-error-format.md)).
- A new `tacit-typecheck` crate wired into `tacit-cli` (`tacit check` subcommand; typecheck integrated into `tacit compile`).
- `--types` / `--effects` view annotation flags.
- Nine-program typed smoke corpus passing under CI. The non-trivial-program exit criterion (sorting, data structure, file I/O beyond `echo`) carried over to Phase 3 per [ADR 0046 § 3](decisions/0046-p2-stage-5-frozen.md) and was closed there.

## What Phase 3 produced (frozen baseline)

Per [ADR 0070](decisions/0070-p3-frozen.md):

- Tacit-Lite primer at `plans/primer/tacit-lite-primer.md` (12,607 `o200k_base` tokens: 10,202 core + 2,405 stdlib appendix).
- 47 hand-authored open-corpus `reference.tac` solutions plus 12 round-2 `reference.stdlib.tac` files; sealed tasks remain Tacit-free per [ADR 0049](decisions/0049-p3-examples-layout-contamination.md).
- 34 `@name` stdlib primitives across nine ADRs (Stage-1 Q-P3-1 surface plus Bundles A–G).
- Three Phase 2 carry-over programs under `examples/phase-3/` (sorting, linked-list, file I/O) — closes [ADR 0046 § 3](decisions/0046-p2-stage-5-frozen.md).
- `corpus-eval` harness with repair-loop, `--include-sealed`, and `--result-label` modes (Anthropic + OpenRouter); Phase 3 metrics schema at `docs/phase-3-metrics.schema.json`.
- Stage 9 baseline + Stage 10 maintenance and cross-family run records under `plans/phase-3-results/`.
- **Strategic finding:** the 30%-below-Python density gate was missed (best Sonnet 61.7%) and structurally miscalibrated (Rust loses to it by +54%). The Python-relative gate is retired; future density work tracks against Rust (Tacit currently 2.92× on the open corpus) as a Phase 4+ aspiration, not a gate. Frontier-model fluency on Tacit-Lite under feedback is established (Sonnet 97.9% library-mediated; GPT-5.4 91.5% primer-only).

## Ground rules

- **Frozen artifacts stay frozen.** The canonical text format ([ADR 0013](decisions/0013-canonical-text-format-frozen.md)), Stage 3 view grammars + AST enum ([ADR 0017](decisions/0017-stage-3-frozen.md)), Stage 5 repo scaffolding ([ADR 0018](decisions/0018-stage-5-frozen.md)), Phase 1 Stages 1–4 ([ADR 0032](decisions/0032-stage-4-frozen.md)), Phase 1 as a whole ([ADR 0033](decisions/0033-phase-1-frozen.md)), Phase 2 Stages 1–4 ([ADR 0044](decisions/0044-p2-stage-1-frozen.md), [ADR 0045](decisions/0045-p2-stage-4-frozen.md)), Phase 2 as a whole ([ADR 0046](decisions/0046-p2-stage-5-frozen.md)), Phase 3 Stage 1 ([ADR 0056](decisions/0056-p3-stage-1-frozen.md)), and Phase 3 as a whole ([ADR 0070](decisions/0070-p3-frozen.md)) are all frozen. Changes require a new ADR and are treated as spec bugs, not scope negotiation.
- **Spec ambiguities are bugs against the relevant frozen artifact.** Resolve via a new ADR, not in-line spec edits.
- **Two views from day one.** Authoring and inspection grammars exist together. Round-trip and rendering tests are the load-bearing checks.
- **Decision log is load-bearing.** Every non-trivial design choice gets an ADR-style entry in `decisions/NNNN-title.md`.

## Key design commitments (do not relitigate)

- Variable references use **DeBruijn indices** in canonical text; no variable IDs. Names are display metadata only.
- Mutual recursion uses explicit `rec { ... }` groupings that hash as a single atom.
- Parser errors produce **typed `Hole` nodes** with structured diagnostics, not failed parses. Hole-node recovery landed in Phase 2 ([ADR 0040](decisions/0040-p2-hole-recovery.md)); the Phase 1 deferral note is [ADR 0023](decisions/0023-hole-node-recovery-deferred.md).
- **BLAKE3** is the hash.
- Display names, comments, and file layout are all sidecar / advisory. The AST is the source of truth.
- Tacit-Lite is the default focus. Tacit-Full features (refinement types, capabilities, handlers) are out of scope for Phases 1–6.
- LLVM 19 is pinned via `inkwell` 0.9's `llvm19-1` feature ([ADR 0032 § 1](decisions/0032-stage-4-frozen.md)). Bumping is a deliberate release-engineering task ([ADR 0031](decisions/0031-llvm-distribution-and-self-hosting.md)).

## Repository layout

```
plans/        — phase plans, specs (canonical-text-format.md, inspection-view.md, sidecar-format.md), primer, test vectors, phase-3 results
docs/         — design docs (compiler-architecture.md, effect-system.md, phase-3-metrics.schema.json)
decisions/    — ADR-style decision log (0001–0071)
crates/       — Cargo workspace: tacit-canonical, tacit-views, tacit-typecheck, tacit-codegen, tacit-cli
examples/     — Phase 1 smoke corpus under smoke/; Phase 3 carry-over programs under phase-3/
corpus/       — Phase 3 evaluation corpus (60 tasks, sealed held-out subset, Tacit references for the open 47)
stdlib/       — libc-effects.toml (Phase 1–2 effect signatures consumed by tacit-typecheck)
```

File extensions per [ADR 0071](decisions/0071-storage-format-reconciliation.md):

| Extension | Role | Checked in |
|-----------|------|------------|
| `.tac`  | Canonical text — byte-exact AST projection, BLAKE3-hashed, authoritative | Yes |
| `.tacd` | JSON display sidecar — binder names, comments, field order, type/effect hints | Yes |
| `.taca` | Authoring view — transient render for human/AI consumption; not produced by the normal dev workflow | Only as historical record (see below) |

**`.taca` exceptions.** Two directory classes have checked-in `.taca` files:
- `corpus/tasks/*/reference.taca` and `examples/phase-3/*.taca` (Mode A) — original authoring-view bytes preserved as the Phase 3 falsification record alongside the canonical `.tac`/`.tacd` pair.
- `plans/phase-3-results/failures/**/generated.taca` (Mode C) — model-generated eval outputs preserved as forensic evidence; not compiled or canonicalized.

CI lives at `.github/workflows/ci.yml`: Python (`uv run pytest`), Rust (`cargo fmt --check`, `cargo clippy --all-targets --features tacit-codegen/llvm19-1,tacit-cli/llvm19-1 -- -D warnings`, `cargo test --features tacit-codegen/llvm19-1,tacit-cli/llvm19-1`), and a CLI smoke step that builds and runs `tacit compile examples/smoke/hello.tac`.

## Open questions

All Phase 0, Phase 1, Phase 2, and Phase 3 open questions are resolved (Q-P3-1 through Q-P3-9 closed by [ADR 0056](decisions/0056-p3-stage-1-frozen.md); the Phase 3 freeze itself is [ADR 0070](decisions/0070-p3-frozen.md)). Phase 4 questions surface as the language-shape work (tuples / records, closures, higher-order combinators) begins; they will be enumerated in `plans/phase-4-plan.md` (not yet created).

## Working style

- Prefer editing existing plan/spec files over creating new ones.
- When a design choice is made, write the ADR before writing the spec text or code that depends on it.
- Phase 4 starts with `plans/phase-4-plan.md` before any implementation. Per [ADR 0070 § Strategic direction](decisions/0070-p3-frozen.md), Phase 4 scope is language-shape work (tuples / records, closures, higher-order combinators) and debugging tooling, justified primarily as "reasoning support" rather than density chase. Phase 4 may not relitigate Python-relative density parity. Resist over-abstracting beyond Phase 4's stated scope.
