# 0058 — Phase 3 closed multi-argument helper lowering

**Status:** Accepted
**Date:** 2026-04-30
**Phase:** 3, Stage 4
**Amended by:** [ADR 0059](0059-p3-rec-hidden-captures.md)

## Context

ADR 0026 kept Phase 1 codegen deliberately small: closed lambdas lower to
top-level monomorphic functions, first-class function values are rejected, and
open lambdas stay out of scope. The initial implementation only emitted unary
`i64 -> i64` functions.

Phase 3 Stage 4 corpus references need ordinary recursive helpers such as
`gcd a b` and `pow base exp`. Keeping those references idiomatic matters
because ADR 0048 and ADR 0057 reject manual unrolling. Encoding every helper
state into one integer is worse than the code it replaces, and for large
integer pairs it is not generally safe within the compiled `i64` subset.

## Decision

Codegen now lowers a consecutive closed lambda chain as one direct-call LLVM
function with matching arity:

```tacit
lambda a. lambda b. body
```

lowers as a private function taking two `i64` parameters. `rec` members may
have different direct-call arities, and every member is still forward-declared
before any body is emitted per ADR 0027.

This does **not** add closures or first-class functions. A function must be
called with all arguments at the call site. Partial application remains a
codegen error.

## Alternatives considered

- **Keep unary lowering and pack state manually.** Rejected. It forces corpus
  references into artificial encodings, violates the Phase 3 idiom rules, and
  fails for general large integer pairs such as Euclidean `gcd`.
- **Add a closure environment ABI.** Rejected. ADR 0026's closure deferral
  still stands; Phase 3 only needs closed direct-call helpers.
- **Add a primitive for each awkward helper.** Rejected. `@gcd` or similar
  would move task logic into the compiler and weaken the evaluation corpus.

## Consequences

- Stage 4 references can use normal recursive helper functions instead of
  manual unrolling.
- Existing unary programs compile unchanged.
- Partial application and open lambdas remain future work; the compiler reports
  arity errors rather than manufacturing closures.
