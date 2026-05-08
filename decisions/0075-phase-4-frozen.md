# 0075 - Phase 4 frozen

**Status:** Accepted
**Date:** 2026-05-08
**Phase:** 4 (exit)
**Closes:** [phase-4-plan.md Stage 9](../plans/phase-4-plan.md)
**Supersedes:** None
**Artifacts frozen by this ADR:**
- [decisions/0072-p4-record-products.md](0072-p4-record-products.md) -
  record products, structural typing, tuple deferral.
- [decisions/0073-p4-function-values-and-closures.md](0073-p4-function-values-and-closures.md) -
  first-class function values, closure ABI, capture/effect rules.
- [decisions/0074-p4-higher-order-combinators.md](0074-p4-higher-order-combinators.md) -
  `@map`, `@fold`, and `@for-each` over `I64Vec`.
- [plans/phase-4-plan.md](../plans/phase-4-plan.md) - all Phase 4 stages
  complete.
- [plans/primer/tacit-lite-primer.md](../plans/primer/tacit-lite-primer.md) -
  Phase 4 primer baseline at 22,157 `o200k_base` tokens.
- [examples/phase-4/](../examples/phase-4/) - durable Phase 4 examples and
  `.tacd` sidecars.
- [plans/phase-4-results/](../plans/phase-4-results/) - Stage 8 open-corpus
  re-evaluation artifacts.

## Context

Phase 4 followed the direction set by [ADR 0070](0070-p3-frozen.md): stop
chasing Python-relative density and address the structural language gaps that
made Phase 3 programs awkward to write and repair. The scoped gaps were product
types, first-class closures, and higher-order combinators. Canonical storage
reconciliation from [ADR 0071](0071-storage-format-reconciliation.md) was
treated as baseline infrastructure: `.tac` is canonical storage, `.tacd` is
display metadata, and `.taca` remains transient authoring text except for
historical artifacts.

Stages 1 through 7 delivered the language surface, diagnostics, inspection
support, primer updates, and examples. Stage 8 then ran an open-corpus
repair-loop re-evaluation without reading, listing, searching, or otherwise
accessing `corpus/sealed/`.

## Delivered language surface

Phase 4 closes the planned Tacit-Lite language-surface slice:

- **Records.** Record construction and projection compile, typecheck, inspect,
  execute, and round-trip. Record types are structural, canonical field layout
  is sorted by field name, and authoring field order remains sidecar metadata.
- **Function values and closures.** Function values lower as two-word closure
  pairs, with code pointer plus immutable environment pointer. Capture sets are
  minimized by free DeBruijn references. Known direct calls remain direct where
  the target is statically known.
- **Closure effects.** Function values carry `fn-ty` call effects inside the
  existing fixed-lattice effect system. Compiler-managed closure storage is not
  exposed as a source-level `Alloc` effect. `Buf` and `I64Vec` handles remain
  non-escapable and cannot be captured by first-class closures.
- **Combinators.** `@map`, `@fold`, and `@for-each` are compiler-recognized
  `@name` forms over `I64Vec` prefixes. Pure and effectful callbacks lower
  through the closure ABI.
- **Inspection and diagnostics.** `tacit view --as inspection --types
  --effects` renders record types, closure capture overlays, and labeled
  combinator blocks. Structured diagnostics cover record shape errors,
  invalid captures, non-function application, callback mismatch, accumulator
  shape, and unsupported collection shape.
- **Durable examples.** `examples/phase-4/` contains canonical examples for
  record accumulators, returned capturing closures, callbacks stored in records,
  and vector combinators.

## Stage 8 re-evaluation

The Phase 4 open run is recorded in
[plans/phase-4-results/](../plans/phase-4-results/):

| Metric | Phase 4 result |
| --- | ---: |
| Run ID | `019e0891-4143-78f6-9146-2c701c408bbb` |
| Provider / model | Anthropic `claude-sonnet-4-6` |
| Scope | open corpus, 47 tasks |
| Repair turns | 2 |
| Primer tokens | 22,157 |
| One-shot task pass rate | 38/47, 80.9% |
| One-shot compile pass rate | 43/47, 91.5% |
| One-shot typecheck pass rate | 44/47, 93.6% |
| Final task pass rate after repair | 47/47, 100.0% |
| Repair recovery | 9/9 initially failed tasks, 100.0% |
| Invalid-output recovery | 4/4, 100.0% |
| Behavioral recovery | 5/5, 100.0% |
| Average model calls per task | 1.23 |
| Total generation tokens | 20,157 |

This is a material fluency improvement over the recorded Phase 3 open
repair-loop baselines:

- Phase 3 core-language repair run `019de6ef-e75e-70d8-aa52-e98c4c577f7d`:
  30/47 one-shot, 40/47 final, 10/17 repairs, 1.53 average model calls.
- Phase 3 library-mediated repair run
  `019df533-fc2a-7511-ad6f-ebdc653878ae`: 32/47 one-shot, 46/47 final, 14/15
  repairs, 1.36 average model calls.

Phase 4 therefore satisfies the fluency non-regression requirement. In
particular, the expanded surface did not confuse the model; it improved
one-shot correctness, final correctness, repair recovery, and call count.

## Density finding

Phase 4 does **not** satisfy the Rust-relative density aspiration under the
recorded end-to-end accounting.

The Stage 8 token counter reports:

| Quantity | Tokens | Ratio vs Rust references |
| --- | ---: | ---: |
| Open Rust references | 7,064 | 1.00x |
| Open canonical Tacit references | 42,376 | 6.00x |
| Phase 4 one-shot model aggregate | 1,057,570 | 149.7x |
| Phase 4 repair model aggregate | 1,305,263 | 184.7x |

The repair aggregate is higher than both recorded Phase 3 comparison points:

- Phase 3 core-language repair aggregate: 1,160,690 tokens.
- Phase 3 library-mediated repair aggregate: 1,255,151 tokens.

The important nuance is that Phase 4 reduced output and repair burden but grew
the primer. Total generation tokens fell to 20,157, below the Phase 3
core-language repair run's 42,314 and the Phase 3 library-mediated run's
24,367. However, the 22,157-token primer is paid once per model call in the
current metric. With 58 total model calls, primer cost dominates the aggregate
and erases the generated-output improvement.

The density result is therefore mixed but not ambiguous:

1. The language surface improved correctness and repair efficiency.
2. The current end-to-end primer-plus-generation density metric worsened.
3. Phase 4 cannot claim the "Rust-relative density narrows" success criterion.
4. More Phase 4 primitives or more primer prose are not justified by this data.

## Decision

**Phase 4 is frozen.** The phase is accepted as a successful language-surface
slice and a negative density finding:

1. Product types, function values, closures, callback effects, and the
   `map`/`fold`/`for-each` combinator family work end to end across parsing,
   canonicalization, views, type/effect checking, codegen, execution,
   diagnostics, and examples.
2. Phase 3 fluency did not regress. The primary Stage 8 open run improved from
   30/47 to 38/47 one-shot versus the Phase 3 core-language repair baseline
   and from 40/47 to 47/47 final after repair.
3. Rust-relative density did not improve under the current end-to-end metric.
   This is recorded as a strategic finding, not a reason to expand Phase 4
   beyond its scoped language surface.
4. The Python-relative gate remains retired. Python-relative numbers may be
   reported descriptively but must not return as a Phase 5+ gate.
5. No sealed-corpus development feedback was used for Phase 4.

## Deferrals

The following remain out of scope after Phase 4:

- Tuple syntax and tuple canonical nodes.
- Record patterns and general product destructuring beyond projection.
- General lists, iterators, hash maps, or collection polymorphism.
- A source-level stdlib/module/import/prelude mechanism.
- Returning freshly allocated `I64Vec` values from `map`.
- General closure capture of non-escapable handles.
- Row polymorphism, user-defined effects, effect handlers, capabilities,
  refinement types, and concurrency.
- Full debugger, structural diff, blame, merge, or Git-driver work.

These deferrals preserve the Phase 7 boundary and leave Phase 5 free to focus
on inspection and debugging tooling rather than another density-driven surface
expansion.

## Consequences

- Phase 5 may begin from a stable Phase 4 language surface.
- Future density work must separate at least four quantities: recurring primer
  cost, generated authoring-view output, canonical storage size, and
  hand-authored reference size. Treating them as one number hides the fact that
  Phase 4 improved generation output while worsening aggregate primer cost.
- The primer should not be expanded further without a targeted reason. The
  Stage 8 result suggests fluency is already strong and primer growth is the
  dominant token cost.
- If a future phase reopens density, it should do so with a metric ADR before
  changing language surface.

