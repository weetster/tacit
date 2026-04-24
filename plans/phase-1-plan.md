# Phase 1 Implementation Plan

**Status:** Draft
**Parent:** [tacit-plan.md](tacit-plan.md)
**Predecessor:** [phase-0-plan.md](phase-0-plan.md) (frozen 2026-04-23)

Phase 1 turns the frozen Phase 0 spec into a working end-to-end pipeline:
canonical-text `.tac` file → typed AST → LLVM IR → native executable, plus
lossless authoring-view round-trip and an inspection-view renderer behind a
`tacit` CLI. Type checking, effects, and optimization are deliberately out of
scope and belong to Phase 2.

## Deliverables (from parent plan § Phase 1)

- Cargo workspace with shared AST crate matching the Phase 0 spec
- Parser for the canonical storage view (reads `.tac` files into AST)
- Parser and serializer for the authoring view, with lossless round-trip
  through canonical text + sidecar
- Inspection-view renderer (display-only per [ADR 0015](../decisions/0015-inspection-view-scope.md))
- LLVM IR emitter for basic constructs: integer arithmetic, function
  definitions, function calls, conditionals, loops
- Minimal libc linkage for hello-world (`printf` or equivalent)
- Hand-crafted test programs in the authoring view
- CLI: `tacit compile foo.tac -o foo`, `tacit view foo.tac --as authoring|inspection`
- Compiler-architecture documentation

## What already exists from Phase 0

- AST enum at [`impls/rs-canonicalizer/src/ast.rs`](../impls/rs-canonicalizer/src/ast.rs)
  — the Stage 3 conforming transcription per [ADR 0016](../decisions/0016-rust-ast-enum-location.md).
- Lexer + parser for canonical text at
  [`impls/rs-canonicalizer/src/lex.rs`](../impls/rs-canonicalizer/src/lex.rs)
  and [`parse.rs`](../impls/rs-canonicalizer/src/parse.rs). Already round-trips
  all 38 `*.canonical` test vectors and rejects every `*.forbidden` /
  `*.reject`.
- Canonical-text emitter and BLAKE3 hasher
  ([`emit.rs`](../impls/rs-canonicalizer/src/emit.rs),
  [`hashing.rs`](../impls/rs-canonicalizer/src/hashing.rs)).

The "parser for canonical storage view" deliverable is therefore largely
*move-and-promote* work, not greenfield. The authoring-view parser, sidecar
I/O, inspection-view renderer, LLVM emitter, and CLI are all greenfield.

## Sequencing

### Stage 1: Cargo workspace + AST crate promotion (~3 days)

Closes the deferred clause from [ADR 0016](../decisions/0016-rust-ast-enum-location.md).

- Introduce a top-level `Cargo.toml` workspace.
- Move the canonical-text crate into the workspace as `tacit-canonical`
  (re-exports `ast`, `lex`, `parse`, `emit`, `hashing`). The existing
  `impls/rs-canonicalizer/` source becomes the new crate's source.
- Wire the existing test-vector tests at the workspace level so the CI job
  defined in [ADR 0018](../decisions/0018-stage-5-frozen.md) keeps passing
  unchanged.
- ADR for the workspace layout (crate names, layering rules: `tacit-canonical`
  has no deps on view/codegen crates).

Stage 1 has no spec changes; it is purely scaffolding. Exit gate: CI green
under the new workspace layout, all 38 canonical-vector hashes still match.

### Stage 2: Authoring view + sidecar round-trip (parallelizable with Stage 3) (~2 weeks)

Greenfield. New crate `tacit-views`.

- **Authoring-view parser** consuming the grammar at
  [`candidates/authoring-bpe-compact.md`](candidates/authoring-bpe-compact.md).
  Output: `(Node, Sidecar)` per the projection rules in that doc's *Projection
  rules* section.
- **Authoring-view serializer** consuming `(Node, Sidecar)` and producing the
  authoring text. Round-trip property: `parse ∘ serialize = id` modulo whitespace
  the grammar declares non-significant.
- **Sidecar reader/writer** for the JSON `.tacd` format defined in
  [`sidecar-format.md`](sidecar-format.md) and [ADR 0014](../decisions/0014-sidecar-format.md),
  including stale-sidecar detection via `targets_hash_blake3` and the
  synthetic-name fallback.
- Property tests over the existing 38 canonical vectors: for each vector,
  emit authoring + sidecar, parse them back, canonicalize, assert byte- and
  hash-identity with the original.

Exit gate: round-trip property holds on every Phase 0 test vector and on
the corpus reference solutions (once Stage 4 below feeds them in via Rust).

### Stage 3: Inspection-view renderer (parallelizable with Stage 2) (~1 week)

Implementation of the spec at [`inspection-view.md`](inspection-view.md), per
[ADR 0015](../decisions/0015-inspection-view-scope.md). Display-only — no
parser, no round-trip claim.

- Render layers L0 (default), L1 (`--debruijn`), L2 (`--hashes`).
- The § 6 worked examples become the regression fixtures: byte-identical
  output is the contract.
- Phase-1+ flags reserved in ADR 0015 (`--types`, `--effects`, `--tree`,
  `--table`) stay stubbed; turning them on is Phase 2+ work.

Exit gate: every § 6 fixture renders byte-identically across L0/L1/L2.

### Stage 4: LLVM IR emitter + libc hello-world (~3 weeks)

The critical-path technical risk for Phase 1. Greenfield, depends only on
Stage 1.

- New crate `tacit-codegen` using `inkwell` (per parent plan § Backend).
- Subset coverage required by Phase 1 exit: integer arithmetic on `i64`,
  `lam` / `app` (closure-free, monomorphic — see open question Q-P1-3),
  `if`, `let`, `rec` (treated as a fixpoint group; see Q-P1-4), `match`
  on integer literal arms, and direct calls into a small libc-wrapper set.
- **libc-wrapper subset (Q3 from parent plan).** Closes Q3 with a Phase 1
  ADR pinning the minimum set: `printf`-equivalent line output, `read`-
  equivalent line input, exit code, byte-string length. Effect signatures
  are written by hand and parked in a dormant table — Phase 2's effect
  system reads them, Phase 1 does not enforce anything.
- Hand-crafted authoring-view programs covering each emitter feature.
  These become the Phase 1 smoke corpus; the Phase 3 evaluation corpus
  stays sealed/untouched per ADR 0020.
- Output path: AST → LLVM IR (textual `.ll` for inspectability) →
  `llc` + system linker → executable. No optimization passes beyond the
  LLVM defaults at `-O0`.

Exit gate: each smoke program canonicalizes, parses, lowers, links, and
runs with the expected stdout. End-to-end test runs in CI on `ubuntu-latest`.

### Stage 5: `tacit` CLI + architecture doc (~1 week)

Depends on Stages 2, 3, 4.

- New crate `tacit-cli` exposing two subcommands:
  - `tacit compile <input.tac> -o <output>` — Stage 4 pipeline.
  - `tacit view <input.tac> --as authoring|inspection [--debruijn] [--hashes]`
    — Stage 2 / Stage 3 renderers.
- Behavior on parse error: surface the existing `ParseError` from
  `tacit-canonical` directly. Typed-`Hole`-node recovery is **deferred to
  Phase 2** — see Q-P1-2; Phase 1 keeps hard failures.
- Architecture doc at `docs/compiler-architecture.md`: crate dependency
  graph, the canonical → AST → IR → object pipeline, where the view system
  attaches, how the sidecar flows through `tacit view`.

Exit gate: a fresh checkout can run `tacit compile examples/hello.tac -o hello
&& ./hello` and the doc explains every box in the data-flow diagram.

## Exit criteria

1. From a `.tac` file written in the authoring view, the user can canonicalize
   it, compile it, run it, and observe expected output. End-to-end CI run
   demonstrates this on every smoke program.
2. `tacit view` round-trips authoring ↔ canonical (via sidecar) on every
   Phase 0 test vector, and renders inspection-view L0/L1/L2 fixtures
   byte-identically.
3. The compiler architecture doc is sufficient that another engineer could
   reproduce the crate layout and pipeline ordering without reading source.

Phase 2 must not begin until all three criteria are met. Spec ambiguities
discovered during Phase 1 are bugs against Phase 0 (per [CLAUDE.md § Ground
rules](../CLAUDE.md)) and resolved with new ADRs against the relevant frozen
artifact, not relitigation in Phase 1.

## Open questions to resolve during Phase 1

Numbered to extend the parent plan's `Q1`–`Q7` scheme.

- **Q-P1-1 — libc-wrapper minimum set (closes parent-plan Q3).** Which
  symbols ship in the Phase 1 stdlib stub, and how are their hand-written
  effect signatures stored so Phase 2's effect checker can pick them up
  without rework? Resolve in Stage 4 ADR.
- **Q-P1-2 — Hole-node parser recovery.** Spec commits to typed `Hole`
  nodes for malformed subtrees ([CLAUDE.md § Key design commitments](../CLAUDE.md)),
  but the existing `tacit-canonical` parser returns `ParseError`. Decide:
  Phase 1 punts (status quo, deferred to Phase 2) vs. Phase 1 retrofits
  (Stage 1 scope creep). Resolve before Stage 1 lands.
- **Q-P1-3 — Closure representation.** Phase 1 emits `lam` / `app`. Decide:
  ban free variables in lambdas (monomorphic top-level only) vs. emit a
  closed-over environment struct now. The simpler choice ships sooner;
  closure ABI changes have downstream cost. Resolve in Stage 4 ADR.
- **Q-P1-4 — Mutual recursion lowering.** `rec` groups hash as a single
  atom and bind N mutually recursive definitions. LLVM-side this is a
  forward-declare + define pattern, but the calling convention and any
  tail-call requirement need to be pinned. Resolve in Stage 4 ADR.
- **Q-P1-5 — Inkwell vs. raw `llvm-sys` vs. textual IR + `llc`.** Parent
  plan names `inkwell`; confirm or override during Stage 4 spike. Pick
  whichever has the lowest ongoing maintenance cost given that Phase 2
  needs richer type and effect metadata in IR.

## Risks

- **LLVM ergonomics are the schedule risk.** `inkwell`'s API surface is
  large and version-coupled to LLVM. Spike before committing to a Stage 4
  schedule.
- **Authoring-view round-trip subtlety.** The grammar deliberately strips
  metadata; the sidecar carries it back. Round-trip failures will show up
  as silent drift in display names. Property tests at Stage 2 exit must
  cover the synthetic-name fallback and stale-sidecar paths, not just the
  happy path.
- **Workspace promotion churn (Stage 1).** Moving the canonicalizer crate
  invalidates every Phase 0 import path. The CI byte-equivalence gate at
  ADR 0018 is the load-bearing check that nothing regressed; do not relax
  it during the move.
- **Hole-node debt.** Deferring Q-P1-2 means Phase 1 ships with a parser
  that diverges from a frozen design commitment. Acceptable iff the
  decision is recorded — the risk is silent drift, not the deferral.
- **Phase 3 corpus contamination.** Phase 1 smoke programs MUST be
  hand-authored, not drawn from `corpus/`. The sealed-hash check from
  [ADR 0020](../decisions/0020-sealing-held-out-in-repo.md) catches
  tampering with sealed tasks but not casual reuse of open ones; treat
  the corpus as read-only throughout Phase 1.
