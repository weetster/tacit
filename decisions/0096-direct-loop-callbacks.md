# 0096 - Direct `@loop` callbacks may access non-escapable handles

**Status:** Accepted
**Date:** 2026-05-18
**Phase:** Stateful host-bridge, Stage 2 corrective amendment
**Amends:** [ADR 0073](0073-p4-function-values-and-closures.md),
[ADR 0085](0085-phase-6-typed-mutable-memory.md), and
[ADR 0093](0093-bounded-stack-loop-primitive.md).

## Context

ADR 0093 added `@loop` as the visible, bounded-stack iteration primitive for
long-running systems loops. Its implementation initially invoked the step
callback through the Phase 4 closure ABI. That preserved the general
higher-order-function model, but it also inherited ADR 0073's capture rule:
first-class closures may not capture region-limited handles such as `Buf`,
`I64Vec`, or typed-vector handles.

That interaction leaves systems code without a good bounded-stack way to loop
over Tacit-owned memory. `rec` helpers may access those handles through
direct-call hidden parameters, but ADR 0093 explicitly keeps `rec` as a
recursion construct that may grow stack. Threading handles through loop state
is also forbidden because loop state is restricted to scalar/fixed-int/record
shapes.

The important distinction is syntactic escape. A lambda written immediately as
the second argument to `@loop` is not stored, returned, or passed through an
ordinary function value boundary. It is a local control-flow body selected by a
compiler-recognized primitive.

No design, implementation, or validation work for this amendment may read,
list, search, or otherwise depend on `corpus/sealed/`.

## Decision

When the second argument of a saturated `@loop init step` application is
syntactically a `lambda` or an annotated `lambda`, the compiler treats that
lambda as a **direct loop callback**, not as a first-class closure value.

Direct loop callbacks:

- have the same source type and effect rule as before:
  `S -> { tag : Int, value : S } / e`;
- are inferred with the loop state as their parameter type, as ADR 0093
  already requires;
- may access non-escapable outer handles from the surrounding scope;
- do not allocate or reify a closure environment;
- lower inline in the `@loop` basic-block loop body;
- remain bounded-stack because the loop back-edge is still an LLVM branch.

The exemption is limited to the immediate callback boundary. Nested lambdas
inside the loop body are still ordinary first-class closure expressions and
must satisfy ADR 0073 capture rules. A callback value supplied through a
variable, record field, function return, partial application, or any other
non-immediate expression also remains an ordinary closure value and may not
capture non-escapable handles.

## Diagnostics

No new diagnostic kind is required. A non-immediate callback that captures a
typed-vector handle still reports `invalid-capture`. A direct callback whose
body does not produce `{ tag : Int, value : S }` still reports
`loop-callback-shape-invalid` or the ordinary type mismatch diagnostics.

Implementations should prefer source spans and binding names for
`invalid-capture`, but that diagnostic improvement is independent of this
language amendment.

## Alternatives considered

### Allow typed-vector captures in all closures

Rejected. That would weaken the anti-escape rule for region-limited handles
and would require a broader ownership/region proof. The immediate `@loop`
callback does not need that generality.

### Guarantee tail-call lowering for `rec`

Rejected for this corrective amendment. ADR 0093 chose a visible iteration
primitive specifically to avoid making stack safety depend on tail-position
analysis through general recursive code.

### Allow typed-vector handles as loop state

Rejected for now. It changes the state representation contract and reopens the
Stage 2 state-shape restriction. Package-instance state and direct callbacks
cover the systems use case without putting handles in the PHI state.

### Add `@loop-with-handle`

Rejected. A new primitive would duplicate `@loop` and force users to choose
between two iteration surfaces. The syntactic immediate-callback rule fixes the
existing primitive directly.

### Extend the same rule to `@map`, `@fold`, and `@for-each`

Deferred. Those combinators can adopt the same direct-callback treatment later,
but `@loop` is the bounded-stack primitive blocking systems-style programs.
Keeping this amendment narrow reduces risk.

## Consequences

- `@loop` becomes usable for long-running loops that read or mutate
  Tacit-owned typed vectors.
- General first-class closure capture rules remain intact.
- Existing programs that pass callback values to `@loop` keep the previous
  closure ABI behavior.
- Codegen can emit fewer closure objects for immediate `@loop` lambdas.
- The primer and workflow prose should eventually describe the immediate
  `@loop` callback as a direct callback, not as a general first-class closure
  capture.
