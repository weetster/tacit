# Phase 4 Plan

**Status:** Active working plan
**Scope:** Tacit-Lite language-surface expansion

## Context

Phase 3 is frozen by [ADR 0070](../decisions/0070-p3-frozen.md). Its
strategic result is that Tacit should lead Phase 4 with reasoning support:
content-addressed structure, typed/effect-tracked programs, and language
shapes that make program intent easier to inspect. Python-relative density is
retired as a gate. Rust-relative density remains a tracked aspiration.

Canonical storage reconciliation is complete and is treated as Phase 4
baseline infrastructure:

| Extension | Role |
| --- | --- |
| `.tac` | Canonical text, byte-exact AST projection, authoritative source |
| `.tacd` | JSON sidecar for display metadata and type/effect hints |
| `.taca` | Transient or historical authoring view only |

All Phase 4 work must preserve that storage model. New tooling must not
reintroduce authoring-view `.tac` files.

## Stage 0 Outcome

Stage 0 scope lock is complete. Phase 4 is active under this plan.

Locked decisions:

- Phase 4 is language-surface work: product types, first-class closures, and
  higher-order combinators.
- Canonical storage reconciliation is baseline infrastructure, not a Phase 4
  gate.
- Phase 7 boundaries are locked: no row polymorphism, handlers,
  user-defined effects, capabilities, refinement types, or concurrency.
- Phase 5 debugging work is not pulled forward, except for inspection and
  diagnostics required to make Phase 4 programs reasoned about.
- Rust-relative density is reported as the primary density comparison. Phase 4
  does not set a numeric Rust-relative gate; measurable movement is expected,
  and failure to move is recorded as a strategic finding in the freeze ADR.

Stage 1 through Stage 7 are complete; Stage 8 may begin with open-corpus
re-evaluation. No agent may read, list, search, or otherwise access
`corpus/sealed/`.

## Goal

Close the dominant remaining structural gap from Phase 3: no product types,
no first-class closures, and no higher-order combinators. The phase should make
Tacit programs easier to structure, inspect, and repair, not merely shorter.

## Deliverables

1. Record product types with structural typing; tuple syntax is deferred by
   [ADR 0072](../decisions/0072-p4-record-products.md).
2. First-class function values and capturing closures.
3. A minimal closure-effect story that remains inside Tacit-Lite.
4. Higher-order combinators, at least `map`, `fold`, and `for-each`.
5. Inspection and diagnostic support sufficient for the expanded surface.
6. Primer updates for the new constructs and idioms.
7. Corpus re-evaluation using Rust-relative density as the primary density
   comparison.
8. Phase 4 freeze ADR.

## Non-Goals

- No Python-relative density gate.
- No Round 3 stdlib expansion as a density strategy.
- No refinement types.
- No effect handlers.
- No user-defined effects.
- No row polymorphism.
- No capabilities.
- No concurrency.
- No general hash maps.
- No module or import system unless it becomes a hard blocker for products or
  closures.
- No full Phase 5 debugger, structural diff, blame, merge, or Git driver work.

New primitives may still be added for correctness or safety-relevant semantic
gaps, but not as a Phase 4 density play.

## Open Questions

| ID | Question | Resolution Point |
| --- | --- | --- |
| Q-P4-1 | Product type choice: tuples, records, or both? | Resolved by [ADR 0072](../decisions/0072-p4-record-products.md): records first, tuples deferred |
| Q-P4-2 | Product syntax, canonical form, projection, destructuring, and patterns | Resolved by [ADR 0072](../decisions/0072-p4-record-products.md): existing `record` / `proj`, projection-based destructuring, no record patterns |
| Q-P4-3 | Closure representation, capture rules, environment layout, and ABI | Resolved by [ADR 0073](../decisions/0073-p4-function-values-and-closures.md): minimized by-value captures, two-word closure pair |
| Q-P4-4 | Function-value effect signatures and closure-captured effects | Resolved by [ADR 0073](../decisions/0073-p4-function-values-and-closures.md): `fn-ty` call effects, no row polymorphism |
| Q-P4-5 | Higher-order combinator surface: primitives or core-language constructs | Stage 5 ADR |
| Q-P4-6 | Testing conventions for larger Tacit programs | Resolved in Stage 6: ADR 0043 remains sufficient for Phase 4 smoke and negative coverage; no new ADR until Stage 7 produces evidence that larger examples need different conventions |

## Required ADR Sequence

1. Product types. Done: [ADR 0072](../decisions/0072-p4-record-products.md).
2. Closure representation and function values. Done:
   [ADR 0073](../decisions/0073-p4-function-values-and-closures.md).
3. Closure effect handling. Done: folded into
   [ADR 0073](../decisions/0073-p4-function-values-and-closures.md).
4. Higher-order combinator surface.
5. Phase 4 testing conventions, if existing conventions are insufficient.
6. Phase 4 freeze.

The ADRs must land before implementation that depends on them.

## Stage 0: Scope Lock

**Status:** Complete

**Purpose:** Turn this plan into the binding Phase 4 scope artifact before
implementation begins.

Work items:

- Confirm that Phase 4 is language-surface work: products, closures, and
  higher-order combinators.
- Record canonical storage reconciliation as complete baseline infrastructure.
- Lock non-goals and Phase 7 boundaries.
- Define the ADR sequence and stage exit checks.
- Decide whether Phase 4 sets a Rust-relative aspiration or only reports
  Rust-relative movement.

Exit criteria:

- `plans/phase-4-plan.md` is accepted as the working plan.
- Q-P4-1 through Q-P4-6 are listed with resolution points.
- No implementation work is blocked on missing scope text.

## Stage 1: Product-Type Design

**Status:** Complete

**Purpose:** Resolve the first Phase 4 design dependency.

Decision summary:

- Records are the Phase 4 product type.
- Tuple syntax and tuple canonical nodes are deferred until records have been
  measured.
- No canonical-format amendment is required for Stage 2.
- Product destructuring is projection-based; record patterns are deferred.

Stage 2 completed record codegen and product diagnostics.

Work items:

- Write the product-type ADR.
- Decide tuples, records, or both.
- Specify canonical representation and hashing behavior.
- Specify authoring-view syntax and inspection-view rendering.
- Specify construction, projection, destructuring, and pattern matching.
- Specify structural typing rules.
- Specify interaction with DeBruijn binding, sidecar display names, and field
  ordering.
- Add canonical and authoring test-vector expectations.

Exit criteria:

- Product-type ADR is accepted.
- Spec text and test vectors are sufficient to implement parser, views,
  typechecker, and codegen without further design choices.

## Stage 2: Product-Type Implementation

**Status:** Complete

**Purpose:** Land products as a complete vertical compiler slice.

Outcome:

- Records lower to LLVM aggregate values with canonical sorted field layout.
- Projection lowers to aggregate extraction by canonical field index.
- Record values can cross `let`, direct lambda calls, and `rec` functions.
- Product diagnostics are structured for duplicate fields, missing fields,
  invalid projections, and record type mismatches.
- Product smoke programs cover construction/projection, function return,
  function argument, nested records, accumulator-style `let`, and `rec`.

Work items:

- Extend canonical parser/emitter only if the ADR requires new canonical
  nodes.
- Extend authoring parser and renderer.
- Extend inspection rendering.
- Extend type inference and structural type checking.
- Implement codegen representation for product construction and projection.
- Add diagnostics for wrong arity, missing fields, duplicate fields,
  incompatible shapes, invalid projection, and record type mismatch.
- Add smoke programs for multi-return, accumulator threading, nested products,
  and product values crossing `let` and `rec`.

Exit criteria:

- Product smoke programs compile, typecheck, execute, and round-trip.
- Existing Phase 1, Phase 2, and Phase 3 smoke programs still pass.
- Product diagnostics are structured and usable by the repair loop.

## Stage 3: Closure And Function-Value Design

**Status:** Complete

**Purpose:** Lift ADR 0026's closed-lambda restriction deliberately.

Decision summary:

- Function values lower as closure pairs: code pointer plus immutable
  environment pointer.
- Capture sets are minimized, by value, and deterministic by DeBruijn index.
- Direct calls for known saturated lambda chains and `rec` members are
  preserved as optimizations.
- Function call effects remain in `fn-ty`; compiler-managed closure storage is
  not a source-level `Alloc` effect.
- `Buf` and `I64Vec` handles remain non-escapable and cannot be captured by
  first-class closures.

Work items:

- Write the closure/function-value ADR.
- Decide closure runtime representation.
- Specify environment capture rules.
- Decide whether capture sets are minimized or deterministic whole-environment
  captures.
- Specify heap, stack, or static environment storage discipline.
- Specify direct-call preservation for closed lambdas and known `rec` members.
- Specify first-class function operations: pass, return, store, and call.
- Specify how function values carry effect signatures.
- Specify how captured effects compose without row polymorphism.
- Specify diagnostics for invalid captures, unsupported escapes, applying
  non-functions, and effect mismatch.

Exit criteria:

- Closure ADR is accepted.
- Function-value and closure-effect rules are precise enough for typecheck and
  codegen implementation.
- Phase 7 features are explicitly excluded.

## Stage 4: Closure And Function-Value Implementation

**Status:** Complete

**Purpose:** Make functions real runtime values.

Outcome:

- Function values lower as two-word closure pairs with typed closure-entry
  functions.
- Non-capturing closures, closures over local first-class values, returned
  closures, and closures stored in records compile and execute.
- Known saturated closed lambda chains and `rec` calls keep direct-call
  lowering.
- Unary `rec` members and partial applications of multi-argument `rec` members
  reify through direct-function adapter closures when their hidden captures are
  escapable.
- First-class closure captures are minimized by free DeBruijn references;
  `Buf` and `I64Vec` captures are rejected as `invalid-capture`.
- Function-typed expressions can be applied through indirect closure calls, and
  applying non-functions produces the structured `apply-non-function`
  diagnostic.
- Stage 4 smoke examples cover non-capturing values, local captures, captured
  function values, returned closures, stored closures, reified `rec` members,
  pure callbacks, and effectful callbacks.

Work items:

- Extend type inference for function values.
- Extend effect checking for function values and captured environments.
- Implement closure conversion in codegen.
- Preserve direct calls where the function target is statically known.
- Support non-capturing function values.
- Support closures over local values.
- Support returned closures where allowed by the ADR.
- Support storing closures in product values where allowed by the ADR.
- Add diagnostics from Stage 3.
- Add tests for pure callbacks, effectful callbacks, nested closures, returned
  closures, stored closures, and invalid applications.

Exit criteria:

- Capturing and non-capturing function values compile, typecheck, execute, and
  round-trip.
- Existing direct-call behavior remains valid.
- Closure diagnostics are structured and stable.

## Stage 5: Higher-Order Combinators

**Status:** Complete

**Purpose:** Land the combinator family that motivated closures.

Outcome:

- `map`, `fold`, and `for-each` are compiler-recognized `@name` forms over
  `I64Vec` prefixes per [ADR 0074](../decisions/0074-p4-higher-order-combinators.md).
- `map` writes into an explicit output vector, `fold` uses accumulator-first
  callbacks, and `for-each` ignores callback results.
- Pure and effectful callbacks lower through the Stage 4 closure ABI.
- Stage 5 smoke examples cover mapping, folding, and effectful traversal;
  the fold example replaces the Phase 3 recursive conceptual-list sum shape.

Work items:

- Write the combinator-surface ADR.
- Decide whether `map`, `fold`, and `for-each` are primitives, syntactic
  forms, or ordinary library-shaped constructs over existing primitives.
- Implement at least `map`, `fold`, and `for-each`.
- Support pure and effectful callbacks.
- Specify callback arities and accumulator/result conventions.
- Add examples that replace manual recursive accumulator threading.
- Add diagnostics for callback type mismatch, callback effect mismatch,
  invalid accumulator shape, and unsupported collection shape.

Exit criteria:

- `map`, `fold`, and `for-each` compile, typecheck, execute, and round-trip.
- Combinators work with effectful callbacks.
- At least one Phase 3 reference pattern is demonstrably simplified by the new
  surface.

## Stage 6: Reasoning And Diagnostics

**Status:** Complete

**Purpose:** Make the expanded surface inspectable without pulling Phase 5
debugging work forward.

Outcome:

- Inspection rendering now makes Phase 4 constructs easier to read:
  record types render structurally under `--types`, capturing lambdas show
  their capture set under `--types` or `--effects`, and full `@map`,
  `@fold`, and `@for-each` applications render as labeled inspection blocks.
- `tacit view --as inspection --types --effects` is covered by Phase 4
  inspection fixtures for records, closures, and combinator applications.
- Typecheck emits the ADR 0073 `invalid-capture` diagnostic for closures that
  capture non-escapable `Buf` or `I64Vec` values, with machine-readable
  capture index and type details.
- Existing product and combinator diagnostics remain the structured repair-loop
  surface for missing fields, record mismatches, callback mismatches,
  accumulator shape errors, and unsupported collection shape errors.
- Testing conventions do not need a new ADR in Stage 6. ADR 0043, as amended
  by ADR 0071 for `.tacd`, remains sufficient for smoke sidecar expectations
  and negative diagnostic coverage. Stage 7 may reopen this only if larger
  examples expose a concrete convention gap.

Work items:

- Extend inspection rendering for products, function types, closures, and
  combinator applications.
- Ensure `tacit view --types --effects` remains useful on Phase 4 examples.
- Improve structured errors for product, closure, and combinator failures.
- Add machine-readable details needed by the corpus repair loop.
- Decide whether testing conventions need an ADR now or can remain deferred.

Exit criteria:

- A failing Phase 4 smoke program can be diagnosed using structured errors and
  `tacit view --types --effects`.
- No full debugger, structural diff, blame, merge, or Git-driver work has been
  pulled into Phase 4.

## Stage 7: Primer And Examples

**Status:** Complete

**Purpose:** Teach the new language surface and create durable examples.

Outcome:

- `plans/primer/tacit-lite-primer.md` now teaches Phase 4 records, capturing
  closures, first-class function values, callback effects, and `@map` /
  `@fold` / `@for-each` over `I64Vec`.
- Durable Phase 4 examples live under `examples/phase-4/` as canonical `.tac`
  files with `.tacd` display sidecars:
  `record-accumulator`, `closure-pipeline`, `stored-callback-record`, and
  `vector-combinators`.
- Typecheck, codegen, and dedicated round-trip smoke coverage now includes the
  Phase 4 examples.
- Primer token count is re-baselined at 22,174 `o200k_base` tokens, measured
  with `tiktoken` via `uv run` from `corpus/harness` on 2026-05-08.

Work items:

- Update `plans/primer/tacit-lite-primer.md`.
- Add sections for products, closures, function values, callback effects, and
  higher-order combinators.
- Add worked examples emphasizing reasoning clarity and structured program
  shape.
- Add Phase 4 examples under `examples/phase-4/`.
- Add or update smoke tests for the new examples.
- Re-baseline primer token count.

Exit criteria:

- Primer examples compile or are clearly marked as explanatory snippets.
- Phase 4 examples compile, typecheck, execute, and round-trip.
- Primer token count is recorded.

## Stage 8: Corpus Re-Evaluation

**Purpose:** Measure whether Phase 4 improves Tacit's structural position
without reviving the retired Python-relative gate.

Work items:

- Promote Rust-relative density as the primary density comparison in reporting.
- Keep Python-relative density descriptive only.
- Re-run open-corpus evaluation against the Phase 3 baseline.
- Report compile success, test pass rate, repair-loop behavior, primer token
  budget, and Rust-relative density delta.
- Track fluency non-regression against Phase 3 results.
- Rewrite a targeted subset of open references only where products, closures,
  or combinators remove known structural friction.

Exit criteria:

- Open-corpus metrics are recorded.
- Phase 3 fluency does not materially regress.
- Rust-relative density improves measurably, or the freeze ADR explains why the
  structural thesis did not translate into density movement.
- No agent reads, lists, searches, or otherwise accesses `corpus/sealed/`.

## Stage 9: Freeze

**Purpose:** Close Phase 4 with an auditable baseline.

Work items:

- Write the Phase 4 freeze ADR.
- Summarize delivered language features.
- Record final smoke and corpus metrics.
- Record any Rust-relative aspiration result.
- Record explicit deferrals to Phase 5+ and Phase 7.
- Update `CLAUDE.md`, `plans/tacit-plan.md`, architecture docs, primer notes,
  and relevant runbooks.
- Run full CI.

Exit criteria:

- Product types compile, typecheck, execute, inspect, and round-trip.
- Capturing closures and first-class function values compile, typecheck,
  execute, inspect, and round-trip.
- `map`, `fold`, and `for-each` compile, typecheck, execute, inspect, and
  round-trip.
- Structured diagnostics cover the new failure modes.
- Existing Phase 1, Phase 2, and Phase 3 behavior is not regressed.
- Open-corpus evaluation is recorded with Rust-relative density as primary.
- Phase 4 freeze ADR is accepted.

## Risks And Mitigations

| Risk | Mitigation |
| --- | --- |
| Record-first product implementation fails to address Phase 3 accumulator threading | Keep tuple syntax deferred, not rejected forever. Re-open with corpus evidence if records do not move the target examples. |
| Closure work expands into Phase 7 effect machinery | Keep the effect story to fixed-lattice sets plus existing basic effect polymorphism. Stop if row polymorphism becomes necessary. |
| Combinators become a second stdlib expansion phase | Gate them on closure semantics and implement only the family needed for Phase 4: `map`, `fold`, and `for-each`. |
| Debugging work expands into Phase 5 | Limit Phase 4 to inspection and diagnostics required for the new surface. |
| Rust-relative density does not improve | Treat that as a strategic finding for the freeze ADR, not as a reason to add unrelated primitives. |
| Expanded syntax harms model fluency | Re-baseline with primer updates and compare against Phase 3 fluency metrics. |

## Final Success Criteria

Phase 4 succeeds when Tacit-Lite has product types, first-class closures, and
the `map`/`fold`/`for-each` combinator family working end to end across parse,
canonicalization, views, type/effect checking, codegen, execution, diagnostics,
examples, and evaluation. The phase should preserve Phase 3 fluency, improve
Rust-relative density on the open corpus, and keep the project positioned
around reasoning support rather than Python token parity.
