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

- AST enum at [`crates/tacit-canonical/src/ast.rs`](../crates/tacit-canonical/src/ast.rs)
  — the Stage 3 conforming transcription per [ADR 0016](../decisions/0016-rust-ast-enum-location.md),
  promoted to the workspace crate `tacit-canonical` per [ADR 0029](../decisions/0029-cargo-workspace-layout.md).
- Lexer + parser for canonical text at
  [`crates/tacit-canonical/src/lex.rs`](../crates/tacit-canonical/src/lex.rs)
  and [`parse.rs`](../crates/tacit-canonical/src/parse.rs). Already round-trips
  all 38 `*.canonical` test vectors and rejects every `*.forbidden` /
  `*.reject`.
- Canonical-text emitter and BLAKE3 hasher
  ([`emit.rs`](../crates/tacit-canonical/src/emit.rs),
  [`hashing.rs`](../crates/tacit-canonical/src/hashing.rs)).

The "parser for canonical storage view" deliverable is therefore largely
*move-and-promote* work, not greenfield. The authoring-view parser, sidecar
I/O, inspection-view renderer, LLVM emitter, and CLI are all greenfield.

## Sequencing

### Stage 1: Cargo workspace + AST crate promotion (~3 days) ✓ DONE 2026-04-24

Closes the deferred clause from [ADR 0016](../decisions/0016-rust-ast-enum-location.md).
Layout decision in [ADR 0029](../decisions/0029-cargo-workspace-layout.md).

- Root `Cargo.toml` workspace at repo root; all Rust crates under `crates/`.
- `impls/rs-canonicalizer/` promoted to `crates/tacit-canonical/` (package
  renamed `tacit-canon` → `tacit-canonical`). Re-exports `ast`, `lex`,
  `parse`, `emit`, `hashing` at crate root. Old directory removed.
- All 14 test-vector tests pass under the new workspace layout; 38 canonical
  vector hashes still match.
- CI job renamed `rs-canonicalizer` → `tacit-canonical`, runs from workspace
  root; cache key updated to `Cargo.lock` at repo root.

Stage 1 has no spec changes; it is purely scaffolding. Exit gate: CI green
under the new workspace layout, all 38 canonical-vector hashes still match.

### Stage 2: Authoring view + sidecar round-trip (parallelizable with Stage 3) (~2 weeks) ✓ DONE 2026-04-24

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

- New crate `tacit-codegen` using `inkwell`
  (per [ADR 0024](../decisions/0024-llvm-bindings-inkwell.md)). LLVM and
  `inkwell` versions pinned in `Cargo.toml`; CI install step added during
  Stage 4.
- Subset coverage required by Phase 1 exit: integer arithmetic on `i64`,
  `lam` / `app` (closed lambdas only — no free variables, top-level
  monomorphic lowering per [ADR 0026](../decisions/0026-phase-1-closed-lambdas.md)),
  `if`, `let`, `rec` (forward-declare-then-define under C calling
  convention per [ADR 0027](../decisions/0027-phase-1-rec-lowering.md)),
  `match` on integer literal arms, and direct calls into the Phase 1
  libc set.
- **libc-wrapper subset** per [ADR 0025](../decisions/0025-phase-1-libc-surface.md):
  `write`, `read`, `exit` — OS-boundary symbols only. Pure-compute
  libc functions are not used (string-literal lengths are compile-time
  constants, block-memory operations emit as LLVM intrinsics). Effect
  signatures live in `stdlib/libc-effects.toml` as a dormant table for
  Phase 2's effect checker; Phase 1 codegen does not consume it. The
  source-level call surface (`@write`, `@read`, `@exit` in authoring
  view; `(sym write|read|exit)` at function position in canonical) is
  fixed by [ADR 0028](../decisions/0028-phase-1-libc-call-surface.md).
- Hand-crafted authoring-view programs covering each emitter feature.
  These become the Phase 1 smoke corpus; the Phase 3 evaluation corpus
  stays sealed/untouched per ADR 0020.
- Output path: AST → `inkwell` `Module` (programmatic IR) → object file
  (in-process emission) → system linker → executable. No optimization
  passes beyond the LLVM defaults at `-O0`. `tacit compile
  --emit-llvm-ir` dumps the constructed `Module` as textual `.ll` for
  debugging, but textual IR is an output only — not a round-trip step.

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

Numbered to extend the parent plan's `Q1`–`Q7` scheme. All five Phase 1
open questions were resolved on 2026-04-24, before Stage 1 landed, via
ADRs 0023–0027.

- **Q-P1-1 — libc-wrapper minimum set (closes parent-plan Q3).**
  Resolved by [ADR 0025](../decisions/0025-phase-1-libc-surface.md):
  Phase 1's libc surface is three OS-boundary symbols (`write`, `read`,
  `exit`). Pure-compute libc functions are not used. Effect signatures
  live in `stdlib/libc-effects.toml` for Phase 2's checker to consume.
- **Q-P1-2 — Hole-node parser recovery.** Resolved by
  [ADR 0023](../decisions/0023-hole-node-recovery-deferred.md):
  Phase 1 keeps hard-failing `ParseError`; typed `Hole` recovery is
  deferred to Phase 2 or a dedicated tooling phase when a concrete
  consumer drives the design.
- **Q-P1-3 — Closure representation.** Resolved by
  [ADR 0026](../decisions/0026-phase-1-closed-lambdas.md): Phase 1
  lambdas must be closed (no free variables). `Lam` lowers as a
  top-level monomorphic LLVM function; `App` lowers as a direct call.
  First-class function values are banned at codegen time. Phase 2+ owns
  the closure ABI design.
- **Q-P1-4 — Mutual recursion lowering.** Resolved by
  [ADR 0027](../decisions/0027-phase-1-rec-lowering.md):
  forward-declare-then-define for all N members of a `Rec` group, LLVM
  default C calling convention (`ccc`) for every Phase 1 function, no
  tail-call optimisation guarantee.
- **Q-P1-5 — Inkwell vs. raw `llvm-sys` vs. textual IR + `llc`.**
  Resolved by [ADR 0024](../decisions/0024-llvm-bindings-inkwell.md):
  `tacit-codegen` uses `inkwell` from the start. Textual IR is available
  as an opt-in `--emit-llvm-ir` CLI dump, not as the load-bearing
  representation. `llvm-sys` reserved as an escape hatch gated by ADR.

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

## Appendix A — Stage 4 worked example (hello-world, end-to-end)

Every box in Stage 4 (authoring text → canonical → AST → `inkwell`
`Module` → object → executable) traced on a single minimal program.
Intended as the reference an implementer can copy-mutate for the rest
of the smoke corpus.

### A.1 Source (authoring view)

File `examples/hello.tac`:

```
@write 1 "hello, world\n" 13
```

Features exercised: integer literal, string literal, four-argument
left-associative juxtaposition (`app`), `@name` primitive-call surface
per [ADR 0028](../decisions/0028-phase-1-libc-call-surface.md). No
binders, no recursion, no `match`. A deliberately small surface for
the first end-to-end run.

### A.2 Canonical text

Left-associative `app` unfolds into three nested `app` nodes. The
`@` is surface-only — it projects to a `sym` node whose canonical
name is the bare identifier. Per canonical-text-format.md § 3, `\n`
in the string literal round-trips as the named escape `\n` (S1), not
the raw LF byte.

```
(app (app (app (sym write) (int 1)) (str "hello, world\n")) (int 13))
```

BLAKE3 of the UTF-8 bytes above is the program's content hash. The
canonical text carries no trailing newline.

### A.3 AST (Rust, `tacit-canonical::ast`)

```rust
Node::App {
    f: Box::new(Node::App {
        f: Box::new(Node::App {
            f: Box::new(Node::Sym("write".into())),
            a: Box::new(Node::Int("1".into())),
        }),
        a: Box::new(Node::Str("hello, world\n".into())),
    }),
    a: Box::new(Node::Int("13".into())),
}
```

(Node-kind names are illustrative — match the actual enum in
`impls/rs-canonicalizer/src/ast.rs`.)

### A.4 Codegen pattern-match (the load-bearing step)

Per [ADR 0028](../decisions/0028-phase-1-libc-call-surface.md),
`tacit-codegen` walks the AST and recognises one Phase 1 shape as a
direct libc call:

> **Shape.** A left-spine of `App` nodes whose leftmost function is
> `Sym(name)` with `name ∈ {"write", "read", "exit"}`. Collect the
> right-spine arguments in source order. Emit a direct `call` to the
> libc symbol of the same name with the collected arguments. A
> `Sym(name)` in function position whose name is outside the
> allowlist fails codegen with `CodegenError::UnknownPrimitive`. All
> other `App` spines lower via the standard closed-lambda path
> ([ADR 0026](../decisions/0026-phase-1-closed-lambdas.md)).

For the hello-world AST, the spine yields symbol `write` and arguments
`[Int 1, Str "hello, world\n", Int 13]`. The string literal lowers to
a private global constant holding the 13 raw bytes
(`h e l l o , SP w o r l d LF`); the integers lower to `i64`
constants; the call target is the external declaration
`declare i64 @write(i32, i8*, i64)`.

### A.5 LLVM IR (what `--emit-llvm-ir` should dump)

```llvm
; ModuleID = 'hello'
target triple = "x86_64-unknown-linux-gnu"  ; ubuntu-latest CI target

@.str.0 = private unnamed_addr constant [13 x i8] c"hello, world\0A"

declare i64 @write(i32, i8*, i64)

define i32 @main() {
entry:
  %buf = getelementptr inbounds [13 x i8], [13 x i8]* @.str.0, i64 0, i64 0
  %_wr = call i64 @write(i32 1, i8* %buf, i64 13)
  ret i32 0
}
```

Notes on this lowering:

- `main` returns `i32 0` rather than calling `exit(0)`. ADR 0025
  permits both; `return 0` is preferred because it keeps the libc
  surface at two symbols (`write`, plus the implicit C runtime entry)
  for hello-world, with `exit` reserved for non-`main` termination.
- The string global uses `\0A` (LF) as raw bytes, not the `\n` named
  escape — textual IR uses its own escape conventions, distinct from
  canonical-text string escapes.
- `call @write` uses LLVM's default C calling convention (`ccc`) per
  [ADR 0027](../decisions/0027-phase-1-rec-lowering.md).
- No optimisation passes; `-O0` default from `inkwell`.

### A.6 Smoke-run (what Stage 4's exit-gate CI job does)

```
$ tacit compile examples/hello.tac -o hello
$ ./hello
hello, world
$ echo $?
0
```

The CI job on `ubuntu-latest` asserts stdout byte-equality (14 bytes
including the trailing LF, since `echo` adds one) and exit code 0 for
each smoke program.

### A.7 Closed: primitive-call surface

Drafting this appendix surfaced the gap that the three libc symbols
had no source-level naming convention. Resolved 2026-04-24 by
[ADR 0028](../decisions/0028-phase-1-libc-call-surface.md): `@name`
in the authoring view, projecting to `(sym name)` in canonical form,
recognised by codegen as a primitive call when the sym is at function
position and the name is in the Phase 1 allowlist. The worked example
above uses the accepted convention.

## Appendix B — Phase 1 smoke corpus (enumerated)

Each program is hand-authored in the authoring view, lives under
`examples/smoke/`, and is wired into the Stage 4 exit-gate CI job.
Programs are listed roughly in order of codegen feature introduction;
implement and test in this order so earlier regressions surface before
later dependencies mask them.

| # | Name | Features exercised | Expected stdout / exit |
|---|---|---|---|
| 1 | `return-zero.tac` | Minimal `main`; no IO; constant return. | stdout empty; exit 0 |
| 2 | `return-computed.tac` | Integer arithmetic (`+`, `-`, `*`) on `i64`; `let`. | stdout empty; exit = computed value (e.g., 42) |
| 3 | `hello.tac` | `@write` primitive, string literal, integer literal, `app` spine. | stdout `hello, world\n`; exit 0 |
| 4 | `if-branch.tac` | `if` / `let` / integer comparison; both arms reachable across two builds. | stdout varies by branch; exit 0 |
| 5 | `factorial.tac` | Self-recursive `rec { fact = lambda n. if n then n * fact (n-1) else 1 }`; exercises forward-declare-then-define on N=1 under [ADR 0027](../decisions/0027-phase-1-rec-lowering.md). | exit = `fact(5) = 120` |
| 6 | `even-odd.tac` | Mutually-recursive `rec { even = ...; odd = ... }`; exercises N=2 rec-group lowering. | exit = 0 or 1 depending on input literal |
| 7 | `match-int.tac` | `match` on integer-literal arms (§ 2 of canonical-text-format.md), fallthrough wildcard. | stdout varies; exit 0 |
| 8 | `echo.tac` | `@read` from fd 0 into a fixed-size buffer, `@write` the same bytes to fd 1. Exercises both IO primitives. | stdout echoes stdin |
| 9 | `exit-nonzero.tac` | Explicit `@exit` with non-zero code from a non-`main` position. | exit 7 (or similar) |

Rules of the smoke corpus:

- **Hand-authored only.** Do not draw from `corpus/` — Phase 3's
  evaluation set stays untouched per [ADR 0020](../decisions/0020-sealing-held-out-in-repo.md).
- **One feature per program where possible.** Feature combinations
  belong in later integration tests, not the smoke set. Isolation
  makes regressions diagnosable.
- **Deterministic output.** No time, no randomness, no environment
  reads. Echo takes fixed stdin from a CI fixture file.
- **No hidden dependencies on stdlib.** Phase 1 has no stdlib; every
  symbol a smoke program references is either a Tacit-Lite AST node
  or one of the three libc entries.
- Primitive-call surface for programs 3, 8, 9 is fixed by
  [ADR 0028](../decisions/0028-phase-1-libc-call-surface.md); no
  sequencing constraint remains.
