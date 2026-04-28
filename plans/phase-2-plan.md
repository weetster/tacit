# Phase 2 Implementation Plan

**Status:** Draft 2026-04-26 · Stage 1 frozen 2026-04-27 ([ADR 0044](../decisions/0044-p2-stage-1-frozen.md)) · Stages 2–4 complete 2026-04-28 ([ADR 0045](../decisions/0045-p2-stage-4-frozen.md)) · Stage 5 complete 2026-04-28 ([ADR 0046](../decisions/0046-p2-stage-5-frozen.md)) · **Phase 2 frozen.**
**Parent:** [tacit-plan.md](tacit-plan.md)
**Predecessor:** [phase-1-plan.md](phase-1-plan.md) (frozen 2026-04-26 — [ADR 0033](../decisions/0033-phase-1-frozen.md))

Phase 2 turns the Phase 1 baseline (canonical AST → LLVM → executable, plus
lossless authoring round-trip and inspection rendering) into a *typed* and
*effect-tracked* baseline. Non-trivial Tacit-Lite programs — sorting, basic
data structures, file I/O — must typecheck with correct effect annotations
and compile end-to-end.

Phase 2 owns three concerns:

1. **A new `tacit-typecheck` crate** wired into `tacit-cli` and into the
   `tacit compile` pipeline ahead of codegen.
2. **Type and effect spec extensions** to the frozen canonical text format,
   the authoring view, and the inspection view — every extension lands as
   an ADR before any code that depends on it (per [CLAUDE.md § Working
   style](../CLAUDE.md)).
3. **The four Phase 1 deferrals** rolled into Phase 2 by
   [ADR 0033](../decisions/0033-phase-1-frozen.md) § 3:
   smoke #7 (`match-int.tac`), smoke #8 (`echo.tac`),
   top-level `module` authoring syntax, and hole-node parser recovery
   ([ADR 0023](../decisions/0023-hole-node-recovery-deferred.md)).

Out of scope by parent plan: effect handlers, user-defined effects, row
polymorphism, refinement types, capabilities. These are Tacit-Full
(Phase 7) and must not be discussed in Phase 2 ADRs except to record the
boundary. The risk register entry "Effect system creep in Phase 2" in
[tacit-plan.md](tacit-plan.md) is binding.

## Deliverables (from parent plan § Phase 2)

- Local type inference within function bodies; explicit type signatures at
  exported definitions.
- Structural type checking; no refinements.
- Basic generic types.
- Simple effect system: fixed lattice (`IO`, `Alloc`, `Mut`, `Div`), local
  inference, mandatory annotations at module boundaries, basic effect
  polymorphism for higher-order functions.
- Effect signatures for the libc-wrapper stdlib, consumed from
  [`stdlib/libc-effects.toml`](../stdlib/libc-effects.toml) (dormant in
  Phase 1, load-bearing in Phase 2 per ADR 0033).
- Structured (JSON-emittable) error reporting format covering both type
  and effect errors.
- Type-directed overload resolution for operators.
- View rendering of effect sets — dense in the authoring view, verbose in
  the inspection view (lights up the `--types` / `--effects` flags
  reserved by [ADR 0015](../decisions/0015-inspection-view-scope.md)).

## Carried over from Phase 1

Per [ADR 0033](../decisions/0033-phase-1-frozen.md) § 3, four deferrals
move to Phase 2 scope. They do not block Phase 2 from starting, but each
needs a Phase 2 ADR before its dependent stage:

- **Smoke #7 — `match-int.tac`.** Blocked on a `pat-int` canonical
  extension. Stage 1 ADR; Stage 4 implementation reaches the codegen
  exit-gate.
- **Smoke #8 — `echo.tac`.** Blocked on a writable-buffer binding model
  ADR. Stage 1 ADR; Stage 4 implementation.
- **Top-level `module` authoring syntax.** Canonical kind exists
  ([ADR 0004](../decisions/0004-rec-arity.md)) and the inspection view
  already renders `module` (Phase 0 Stage 3); only the authoring-view
  surface is held back. Stage 1 ADR; Stage 4 implementation closes the
  exclusion in [phase-1-plan.md § Stage 2](phase-1-plan.md).
- **Hole-node parser recovery ([ADR 0023](../decisions/0023-hole-node-recovery-deferred.md)).**
  Phase 1 hard-fails with `ParseError`; Phase 2 emits typed `Hole` nodes
  with structured diagnostics so type and effect checking continue past
  malformed subtrees. Stage 1 ADR; Stage 4 implementation.

## What already exists from Phase 1

- Frozen canonical text format ([ADR 0013](../decisions/0013-canonical-text-format-frozen.md)),
  AST enum at [`crates/tacit-canonical/src/ast.rs`](../crates/tacit-canonical/src/ast.rs),
  view grammars ([ADR 0017](../decisions/0017-stage-3-frozen.md)), and
  the Phase 1 codegen subset ([ADR 0032](../decisions/0032-stage-4-frozen.md)).
- Crate graph (per [docs/compiler-architecture.md](../docs/compiler-architecture.md)):
  `tacit-canonical` → `tacit-views`, `tacit-canonical` → `tacit-codegen`,
  both → `tacit-cli`. Phase 2 adds `tacit-typecheck` between `tacit-canonical`
  and `tacit-codegen` in the compile pipeline.
- [`stdlib/libc-effects.toml`](../stdlib/libc-effects.toml) populated with
  `write`, `read`, `exit`. Schema and contents frozen by
  [ADR 0025](../decisions/0025-phase-1-libc-surface.md); Phase 2 is the
  first consumer.
- Phase 1 smoke corpus (`return-zero`, `return-computed`, `hello`,
  `if-branch`, `factorial`, `even-odd`, `exit-nonzero`) under
  [`examples/smoke/`](../examples/smoke/) — these are the regression
  baseline; Phase 2 must not regress them, and must add typed signatures
  for each as the typechecker comes online.

## Sequencing

The pattern follows Phase 1: spec freezes first, then implementation
stages each gated by an exit criterion. Stage 1 is sequencing-critical;
Stages 2–4 may overlap once Stage 1 ADRs land.

### Stage 1: Spec ADRs — type/effect surface + Phase 1 carry-overs (~3–4 weeks)

**Status: Frozen 2026-04-27 by [ADR 0044](../decisions/0044-p2-stage-1-frozen.md).** All ten Q-P2-N items below have Accepted ADRs (0034–0043); test vectors V29–V33 are committed under [`plans/test-vectors/`](test-vectors/).

ADRs only. No production code. Stage 1 closes every open spec question
that Stages 2–5 would otherwise have to bikeshed mid-implementation. Each
ADR is a separate decision with its own freeze gate; this stage is
"complete" when every Q-P2-N below has an Accepted ADR.

Open questions, numbered to extend the parent-plan / phase-1-plan
`Q-PN-N` scheme:

- **Q-P2-1 — Type subset for `ann`.** Closes the [canonical-text-format.md
  § 11](canonical-text-format.md#11-open-items) "Type syntax inside `ann`"
  open item. Enumerates the expression kinds that are valid in type
  position: base type symbols, function arrows, record types, generic
  type variables (DeBruijn-indexed at the type level). Updates the canonical
  spec via amendment ADR per the [CLAUDE.md ground rule](../CLAUDE.md) that
  spec ambiguities are bugs against the relevant frozen artifact. V29 in
  [`plans/test-vectors/`](test-vectors/) (currently blocked) becomes the
  first conformance vector.
- **Q-P2-2 — Effect-set canonical syntax + lattice ordering.** How effect
  sets appear inside type expressions; canonical sort order for sets so
  hash-equality of semantic-equality holds (mirrors
  [ADR 0008](../decisions/0008-record-field-ordering.md)); subsumption /
  joining rules for the fixed `IO`/`Alloc`/`Mut`/`Div` lattice.
- **Q-P2-3 — Effect polymorphism surface syntax.** Closes parent-plan Q2.
  How effect variables appear in canonical, authoring, and inspection
  forms — especially in higher-order signatures like
  `map :: (a → b / e) → [a] → [b] / e`. Per
  [docs/effect-system.md](../docs/effect-system.md), "basic" means one
  effect variable per function; row polymorphism is Tacit-Full and out
  of scope.
- **Q-P2-4 — `pat-int` canonical extension.** Closes
  [ADR 0032 § 3](../decisions/0032-stage-4-frozen.md). Adds an
  integer-literal pattern kind to canonical and the `match` arms.
  Unblocks smoke #7.
- **Q-P2-5 — Writable-buffer binding model.** Closes ADR 0032 § 3.
  How a `read` destination is named, sized, and lifetimes-scoped so
  the existing closed-lambda + `let` discipline
  ([ADR 0026](../decisions/0026-phase-1-closed-lambdas.md)) extends
  cleanly. Must compose with the `Mut` and `Alloc` effects from Q-P2-2.
  Unblocks smoke #8.
- **Q-P2-6 — Top-level `module` authoring syntax.** Closes the
  [phase-1-plan.md Stage 2](phase-1-plan.md) exclusion. Canonical
  `module` is frozen ([ADR 0004](../decisions/0004-rec-arity.md)); only
  the authoring projection is open. Defines export-boundary annotation
  syntax used by Q-P2-1 and Q-P2-2.
- **Q-P2-7 — Hole-node parser recovery.** Supersedes
  [ADR 0023](../decisions/0023-hole-node-recovery-deferred.md). Defines
  recovery boundaries in the authoring parser, the diag-id set
  (extends the § 8 starter table from canonical-text-format.md), and the
  payload contract that downstream typecheck / codegen passes consume
  to skip past holes without spurious cascading errors.
- **Q-P2-8 — Structured error format.** JSON schema for type and effect
  errors, shared with hole diagnostics from Q-P2-7. AST-path addressing,
  expected-vs-actual structure, candidate-fix slot per the parent plan's
  "errors are structured data" commitment.
- **Q-P2-9 — Operator overload resolution.** Type-directed dispatch rules
  for arithmetic and comparison primitives
  ([ADR 0030](../decisions/0030-phase-1-arith-primitives.md)) once
  numeric width types are in scope. Width inference vs. explicit
  annotation defaults; coercion is forbidden by parent plan, so this is
  a *resolution* discipline, not a coercion one.
- **Q-P2-10 — Test conventions for typed programs.** Closes parent-plan
  Q4 to the extent Phase 2 needs it. Whether the smoke corpus carries
  per-program type / effect expectations alongside stdout / exit-code
  expectations.

Exit gate: every Q-P2-N has an Accepted ADR; the canonical-text-format
amendment ADRs (Q-P2-1, -2, -4, -5) ship with conformance test vectors
landed under [`plans/test-vectors/`](test-vectors/) and passing on both
the existing canonical-form parser and the new typecheck consumer
(consumer can be a stub at this stage).

### Stage 2: `tacit-typecheck` crate + structural type checker (~3–4 weeks)

**Status: Complete 2026-04-27.** All seven smoke programs typecheck; negative-test corpus covers every error variant; crate builds without an LLVM feature flag.

Greenfield crate. Depends on Stage 1 ADRs Q-P2-1, -8, -9, -10 (and -7
to the extent Phase 2 wants Hole nodes to flow through cleanly; see
note below). Independent of Stage 3's effect work — types first, effects
on top.

- New crate `tacit-typecheck` under `crates/tacit-typecheck/`. Public
  surface: `infer_module(node) -> Result<TypedModule, Vec<TypeError>>`
  and a JSON-emittable `TypeError` per Q-P2-8.
- Structural typing: types are compared by shape, not name.
- Local inference inside function bodies; exported definitions in a
  `module` carry explicit signatures (per parent plan § Phase 2).
- Basic generics: parametric type variables, no constraints, no
  higher-kinded types.
- Operator overload resolution per Q-P2-9.
- All seven Phase 1 smoke programs gain explicit signatures and
  typecheck under the new crate. Failures here are bugs against
  Stage 1 ADRs, not against Phase 1 codegen.
- Hole-node interaction: if Q-P2-7 is on the critical path, typecheck
  treats `Hole` as a type-error stop with the diag forwarded; otherwise
  Phase 1's hard-fail behavior is preserved and Stage 4 retrofits
  Hole-aware typecheck. Decide at Stage 2 entry based on Q-P2-7 status.

Exit gate: every Phase 1 smoke program typechecks; the JSON error
format from Q-P2-8 is exercised by a deliberate negative-test corpus
(at least one case per error variant); `tacit-typecheck` builds and
tests without an LLVM feature flag (parallel to the
`tacit-codegen::analysis` layer in
[docs/compiler-architecture.md](../docs/compiler-architecture.md)).

### Stage 3: Effect checker (~3 weeks)

**Status: Complete 2026-04-27.** All seven smoke programs have verified effect signatures; effect polymorphism propagates through higher-order functions; pure-annotation-on-IO-body produces `effect-violation`. Inspection-view `--effects` rendering deferred to Stage 5 (view annotations).

Sits on top of Stage 2 inside the same `tacit-typecheck` crate. Depends
on Stage 1 ADRs Q-P2-2 and Q-P2-3.

- Fixed atomic effects: `IO`, `Alloc`, `Mut`, `Div`. No new atoms.
  No `Exn` (parent plan § Decisions baked in).
- Effect inference inside function bodies: union of callees + primitive
  effect contributions.
- Mandatory effect annotations on exported definitions in `module`
  (parent plan § Decisions baked in).
- Basic effect polymorphism for higher-order functions per Q-P2-3.
- Consume [`stdlib/libc-effects.toml`](../stdlib/libc-effects.toml) for
  `write` / `read` / `exit` effect signatures. The toml file becomes
  the source of truth for primitive effect sets; codegen never reads
  it (codegen still uses the
  [ADR 0028](../decisions/0028-phase-1-libc-call-surface.md) allowlist
  for symbol-name dispatch).
- Effect-set rendering in the inspection view (`--effects` flag,
  reserved by [ADR 0015](../decisions/0015-inspection-view-scope.md)).
- Refuses to relax: row polymorphism, handlers, and user-defined
  effects are scope violations, not future work to design ahead.

Exit gate: every Phase 1 smoke program has a verified effect signature
matching its observable behavior (`hello.tac` → `IO`,
`exit-nonzero.tac` → `IO`, `factorial.tac` → `{}` (pure), `even-odd.tac`
→ `Div`, etc.); a test program using `map` over an `IO` callback
typechecks with the propagated effect; a test program annotating purity
on a body that contains `@write` produces the expected effect-error
JSON.

### Stage 4: Hole recovery + `module` authoring + Phase 1 carry-overs (~2–3 weeks)

**Status: Complete 2026-04-28. Frozen by [ADR 0045](../decisions/0045-p2-stage-4-frozen.md).** Nine-program smoke corpus passes; module round-trip and pat-int round-trip pass; hole recovery flows through to typecheck diagnostics without hard-fail; clippy clean.

Closes the four Phase 1 deferrals as concrete features. Depends on
Stage 1 ADRs Q-P2-4, -5, -6, -7. Independent of Stage 3 except where
the writable-buffer model (Q-P2-5) interacts with effects.

- **Hole recovery** in the authoring parser. `tacit-views::authoring`
  emits `Hole` nodes per Q-P2-7 instead of `ParseError`. Inspection
  view already renders them; round-trip remains lossy through
  authoring per phase-1-plan.md Stage 2.
- **Top-level `module` authoring syntax** per Q-P2-6. The previously
  excluded fixtures in
  [`crates/tacit-views/tests/round_trip.rs`](../crates/tacit-views/tests/round_trip.rs)
  move into the round-trip property.
- **Smoke #7 — `match-int.tac`** lowers, links, and runs. Codegen for
  `pat-int` extends the
  [ADR 0032 § 3](../decisions/0032-stage-4-frozen.md) `match`
  pattern-matching path.
- **Smoke #8 — `echo.tac`** lowers, links, and runs. Writable-buffer
  binding lowers per Q-P2-5; effect set is `IO ∪ Mut` per Q-P2-2.

Exit gate: nine-program smoke corpus runs end-to-end on
`ubuntu-latest` CI; the round-trip property in `tacit-views` covers
the previously excluded `module`-bearing fixtures; a parser-error
fixture demonstrates `Hole` flowing through to a structured-JSON
diagnostic without a hard fail.

### Stage 5: `tacit-cli` wiring + view annotations + architecture doc + freeze (~1–2 weeks)

**Status: Complete 2026-04-28. Frozen by [ADR 0046](../decisions/0046-p2-stage-5-frozen.md).** `tacit check` works without LLVM; `tacit compile` runs typecheck ahead of codegen (exit 1 for type/effect errors, exit 2 for codegen/linker failures); `--types` and `--effects` flags wired in inspection view; architecture doc updated with Phase 2 crate graph, pipelines, and codegen-subset table; clippy clean.

Threads the typechecker into the CLI and updates the load-bearing
architecture doc. Depends on Stages 2–4.

- `tacit compile` runs `tacit-typecheck::infer_module` ahead of
  `tacit-codegen::compile_to_object`. Type and effect errors abort
  compile with structured JSON diagnostics on stderr, exit code
  separate from codegen failures.
- New subcommand `tacit check <input.tac>` runs typecheck only;
  emits diagnostics to stdout as JSON or to stderr as human-readable
  text, picked by `--format json|text` (default `text`). No codegen
  dependency, so `tacit check` works in builds without an LLVM
  feature flag.
- `tacit view --as inspection` honors `--types` and `--effects` flags
  (currently stubbed per [ADR 0015](../decisions/0015-inspection-view-scope.md)).
- Architecture doc at
  [`docs/compiler-architecture.md`](../docs/compiler-architecture.md):
  add the `tacit-typecheck` crate to the dependency graph; add the
  `tacit check` and revised `tacit compile` data-flow diagrams; record
  the libc-effects.toml consumer; add a Phase 2 codegen-subset table
  if Stage 4 expands the lowered AST shapes.
- A Phase 2 freeze ADR mirroring
  [ADR 0033](../decisions/0033-phase-1-frozen.md) records what was
  built and what is deferred to Phase 3.

Exit gate: a fresh checkout runs `tacit check examples/smoke/echo.tac`
and gets a clean pass; `tacit compile examples/smoke/echo.tac -o echo
&& echo hi | ./echo` echoes correctly; CI runs `cargo test
--features tacit-codegen/llvm19-1,tacit-cli/llvm19-1` plus the new
typecheck negative-test corpus; the doc explains every new box in the
data-flow diagram.

## Exit criteria

Per parent plan § Phase 2:

> Non-trivial programs (sorting algorithms, basic data structures, file
> I/O) typecheck with correct effect annotations and compile.

Concretely:

1. **Typed smoke corpus.** All nine smoke programs (Phase 1's seven plus
   #7 and #8) carry signatures, typecheck, lower, link, and produce the
   expected stdout / exit code under CI.
2. **Non-trivial programs.** A small Phase 2 program set — at least one
   sorting algorithm, one linked-list-style data structure, and one
   file-IO program beyond `echo` — typechecks with correct effect
   annotations and compiles. Lives under `examples/phase-2/`, hand-authored
   per the same rules as the Phase 1 smoke corpus
   ([phase-1-plan.md § Appendix B](phase-1-plan.md#appendix-b--phase-1-smoke-corpus-enumerated)),
   never drawn from `corpus/`.
3. **Round-trip and inspection gates from Phase 1 hold.** No regression
   to the
   [ADR 0033](../decisions/0033-phase-1-frozen.md) authoring ↔ canonical
   round-trip property or to the L0/L1/L2 inspection-view fixtures.
   Newly added view flags (`--types`, `--effects`, and module syntax)
   land with their own fixtures.
4. **Structured diagnostics.** A negative-test corpus exercises every
   `TypeError` and `EffectError` variant under the Q-P2-8 JSON schema;
   `Hole` diagnostics from Q-P2-7 share that schema.

Phase 3 must not begin until all four are met. Spec ambiguities
discovered during Phase 2 are bugs against either Phase 0 or the Stage 1
ADRs (per [CLAUDE.md § Ground rules](../CLAUDE.md)) and are resolved with
new ADRs, not by relitigating the relevant frozen artifact.

## Risks

- **Effect-system creep.** The named risk in the parent plan's risk
  register. Mitigation: Stage 1 ADR scope is fixed at the
  `IO`/`Alloc`/`Mut`/`Div` lattice with one effect variable per function.
  If a Stage 3 design pressure asks for handlers, row polymorphism, or
  user-defined effects, that design pressure is a Phase 7 signal — stop
  and defer, do not extend Phase 2.
- **Type-inference scope creep.** Hindley-Milner-style inference is a
  research-grade design surface; Phase 2 explicitly asks for
  *local* inference plus mandatory annotations at module boundaries.
  If a non-trivial program requires whole-module inference to
  typecheck, the program is wrong (parent plan § Decisions baked in:
  every relevant fact about a function should be derivable from its
  signature without whole-program analysis).
- **Spec-ADR backlog blocking implementation.** Stage 1 has nine ADRs.
  Mitigation: ADRs are independent and can land in parallel; only
  Stage 4's smoke #7 / #8 / module / hole work is fully blocked
  on its specific ADRs. Stages 2 and 3 only need Q-P2-1, -2, -3, -8,
  -9, -10 and can begin once those land.
- **`stdlib/libc-effects.toml` schema drift.** The toml schema is
  frozen by [ADR 0025](../decisions/0025-phase-1-libc-surface.md). If
  Stage 3's checker needs more fields (e.g., effect-polymorphic
  variants, error-channel info), that is a new ADR superseding 0025,
  not an in-place schema edit.
- **Round-trip regression from new authoring syntax.** Stage 4's
  `module` and Hole work expand the authoring grammar. Mitigation:
  the round-trip property in
  [`crates/tacit-views/tests/round_trip.rs`](../crates/tacit-views/tests/round_trip.rs)
  is the load-bearing check; any regression there is a hard stop.
- **Phase 3 corpus contamination.** Phase 2 `examples/phase-2/`
  programs MUST be hand-authored, not drawn from `corpus/`. Same rule
  as Phase 1 smoke corpus per
  [ADR 0020](../decisions/0020-sealing-held-out-in-repo.md).

## See also

- [tacit-plan.md § Phase 2](tacit-plan.md) — parent plan deliverable list.
- [docs/effect-system.md](../docs/effect-system.md) — Lite vs. Full
  boundary; the Phase 2 / Phase 7 seam in human-readable form.
- [ADR 0023](../decisions/0023-hole-node-recovery-deferred.md) — hole
  recovery deferral that Q-P2-7 closes.
- [ADR 0025](../decisions/0025-phase-1-libc-surface.md) —
  libc-effects.toml schema; Phase 2's primary spec input.
- [ADR 0032](../decisions/0032-stage-4-frozen.md) § 3 — smoke #7 / #8
  deferral that Stage 4 closes.
- [ADR 0033](../decisions/0033-phase-1-frozen.md) — Phase 1 baseline
  this plan is layered onto.
