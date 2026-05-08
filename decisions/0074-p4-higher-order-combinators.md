# 0074 - Phase 4 higher-order combinators

**Status:** Accepted
**Date:** 2026-05-07
**Phase:** 4, Stage 5
**Closes:** [phase-4-plan.md Q-P4-5](../plans/phase-4-plan.md)
**Affirms:** [ADR 0036](0036-p2-effect-polymorphism-syntax.md),
[ADR 0061](0061-p3-stdlib-bundle-a-i64-vectors.md),
[ADR 0073](0073-p4-function-values-and-closures.md)
**Amends:** [ADR 0041](0041-p2-structured-error-format.md) - additive
diagnostic kinds

## Context

Phase 4 added first-class closures so Tacit programs can stop spelling every
collection traversal as a local recursive helper. The remaining Stage 5
question is where `map`, `fold`, and `for-each` live in the surface.

Tacit-Lite does not yet have a general list type, module/import system, row
polymorphism, or region-safe first-class mutable handles. The executable
collection surface available today is `I64Vec`: it is already supported by
typecheck and codegen, but remains a non-escapable handle per ADR 0061 and
ADR 0073.

## Decision

**Phase 4 Stage 5 adds compiler-recognized `@map`, `@fold`, and `@for-each`
combinators over `I64Vec`. They are primitive-shaped `sym` heads, not new
canonical nodes or a general stdlib mechanism.**

No canonical-format amendment is required. The surface forms are:

```tacit
@map xs count callback out
@fold xs count init callback
@for-each xs count callback
```

### Collection shape

The only Stage 5 collection shape is an `I64Vec` prefix:

- `xs` is an `I64Vec`;
- `count` is an `Int`;
- indices visited are `0 .. count - 1`;
- negative `count` behaves as an empty prefix because the loop condition is
  `i < count` starting at `0`.

`@map` writes results to an explicit output `I64Vec` because `I64Vec` handles
are not first-class return values. `out` must have capacity for at least
`count` elements; bounds checking remains outside Tacit-Lite Stage 5, matching
the existing `@i64-get` and `@i64-set` primitives.

### Callback conventions

Callbacks operate on integer elements:

| Combinator | Callback shape | Result |
| --- | --- | --- |
| `@map xs count f out` | `Int -> Int / e` | writes `f xs[i]` into `out[i]`, returns `0` |
| `@fold xs count init f` | `Int -> Int -> Int / e` | returns final accumulator |
| `@for-each xs count f` | `Int -> Int / e` | calls `f xs[i]`, ignores callback result, returns `0` |

For `fold`, the callback receives the accumulator first and the element
second: `f acc elem`. The first curried application must be pure; the final
application carries the callback effect `e`. This keeps the callback inside
ADR 0036's one-effect-variable model without row polymorphism.

### Effects

Evaluating a callback value is still governed by ADR 0073. Calling the
callback inside a combinator contributes the callback call effect.

`@map` also has `Mut` because it writes `out`. `@fold` and `@for-each` add no
effect of their own beyond evaluating their arguments and calling the callback.
An effectful callback is valid as long as its function type expresses that
effect.

### Diagnostics

Stage 5 adds these structured diagnostic kinds:

| Kind | Meaning |
| --- | --- |
| `callback-type-mismatch` | callback is not the required unary or binary integer function shape |
| `callback-effect-mismatch` | `fold` callback performs effects on the first curried application |
| `invalid-accumulator-shape` | `fold` accumulator value or callback accumulator/result is not `Int` |
| `unsupported-collection-shape` | combinator collection argument is not an `I64Vec` |

Existing `type-mismatch` remains valid for ordinary non-callback arguments
such as `count`.

## Alternatives considered

- **New canonical nodes.** Rejected. `app` plus `sym` already expresses the
  surface, and Stage 5 does not need syntax with new binding or hashing
  behavior.
- **Ordinary library definitions.** Rejected for Phase 4. Tacit has no module
  or import system, and `I64Vec` handles cannot be returned or captured as
  first-class values. Compiler recognition is the narrow vertical slice that
  exercises closures without pretending a stdlib exists.
- **General collection polymorphism.** Rejected. There is no executable
  first-class list type yet, and row polymorphism remains Phase 7 scope.
- **Return a freshly allocated vector from `map`.** Rejected. That would make
  `I64Vec` a first-class escaping value or require ownership/lifetime rules
  outside Phase 4.
- **Allow effectful first applications in `fold` callbacks.** Rejected. It
  would require composing multiple callback call effects in a curried function
  shape. The Stage 5 convention keeps effects on the final element-consuming
  application.

## Consequences

- Stage 5 closes the closure-motivated combinator surface without expanding
  into a second stdlib phase.
- `I64Vec` remains non-escapable. Callbacks receive elements and accumulators,
  not collection handles.
- `map`, `fold`, and `for-each` can be used from canonical `.tac` files and
  authoring `.taca` input through the existing `@name` syntax.
- A later phase can add list types, iterator sugar, or library-defined
  combinators without changing this canonical representation.

## Related decisions

- [ADR 0036](0036-p2-effect-polymorphism-syntax.md) - one effect variable for
  higher-order functions.
- [ADR 0041](0041-p2-structured-error-format.md) - structured diagnostics.
- [ADR 0061](0061-p3-stdlib-bundle-a-i64-vectors.md) - `I64Vec` handle model.
- [ADR 0073](0073-p4-function-values-and-closures.md) - closure ABI and
  callback effect rules.
