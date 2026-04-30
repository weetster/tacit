# 0059 — Phase 3 `rec` hidden capture parameters

**Status:** Accepted
**Date:** 2026-04-30
**Phase:** 3, Stage 5
**Amends:** [ADR 0058](0058-p3-closed-multi-arg-helper-lowering.md)

## Context

ADR 0058 intentionally stopped short of a closure ABI: helper lambdas lower
as private direct-call functions, and callers must supply every source-level
argument. Stage 5 collection and algorithm references add another direct-call
need. Recursive helpers often need access to an input buffer or to runtime
values computed before the `rec` block, for example a line length or parsed
target value.

The initial lowering reused those outer LLVM values directly inside the
hoisted helper function. That is invalid IR: an instruction defined in
`main` cannot be referenced from a different private function. Rewriting all
Stage 5 references to avoid captured buffers would force artificial packing or
test-specific unrolling, violating ADR 0048's reference idiom rules more than
a small direct-call lowering extension does.

## Decision

`rec` members may capture outer runtime values and stack-buffer pointers, but
only through hidden direct-call parameters.

Concretely:

- Each `rec` member is still a private function with all source-level lambda
  parameters first.
- The compiler appends hidden parameters for every outer `Value` (`i64`) and
  `Ptr` (`ptr`) binding visible at the `rec` site. Outer `Function` bindings
  remain direct symbol references and are not passed as hidden parameters.
- Calls to a `rec` member append the current capture values after the
  source-level arguments.
- Inside a `rec` member body, the captured bindings occupy the same DeBruijn
  positions as the original outer environment, so authoring-view source and
  type inference are unchanged.
- Captures are currently the whole visible non-function environment, not a
  minimized free-variable set. This keeps codegen simple and deterministic;
  unused hidden parameters are acceptable at Phase 3 scale.

This is not a first-class closure representation. Captured `rec` members are
still only callable directly at known call sites. Partial application and
passing functions as values remain unsupported.

## Alternatives considered

- **Keep Stage 5 references closed-only.** Rejected. Buffer-heavy collection
  and sorting references would have to encode input into large integers or
  duplicate tests by hand, which is worse for the evaluation corpus.
- **Add a full closure environment ABI.** Rejected. This would reopen ADR
  0026's first-class-function deferral. Stage 5 needs direct helper calls,
  not function values.
- **Minimize hidden captures with free-variable analysis.** Rejected for now.
  It is an optimization. Passing the whole visible value/pointer environment
  is simpler and preserves behavior.

## Consequences

- Stage 5 references can use ordinary recursive helpers over input buffers and
  parsed runtime values.
- LLVM IR no longer contains cross-function instruction references for
  captured values.
- Function signatures may contain unused hidden parameters. This is noisy in
  IR but has no semantic effect.
- Future closure work can still introduce a real environment object without
  changing the source-level `rec` syntax.

## Related decisions

- [ADR 0026](0026-phase-1-closed-lambdas.md) — first-class closures remain
  out of scope.
- [ADR 0027](0027-phase-1-rec-lowering.md) — `rec` members still use
  forward-declared private functions under C calling convention.
- [ADR 0048](0048-p3-tacit-idiom-rules.md) — references should use clear
  recursive helpers rather than artificial encodings.
- [ADR 0058](0058-p3-closed-multi-arg-helper-lowering.md) — direct-call
  multi-argument helper lowering amended by this ADR.
