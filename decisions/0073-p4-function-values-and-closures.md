# 0073 - Phase 4 function values and closures

**Status:** Accepted
**Date:** 2026-05-07
**Phase:** 4, Stage 3
**Closes:** [phase-4-plan.md Q-P4-3, Q-P4-4](../plans/phase-4-plan.md)
**Supersedes in part:** [ADR 0026](0026-phase-1-closed-lambdas.md) - the
closed-lambda and no-first-class-function restrictions
**Amends:** [ADR 0041](0041-p2-structured-error-format.md) - additive
diagnostic kinds
**Affirms:** [ADR 0034](0034-p2-type-subset-ann.md),
[ADR 0035](0035-p2-effect-set-canonical.md),
[ADR 0036](0036-p2-effect-polymorphism-syntax.md),
[ADR 0038](0038-p2-writable-buffer.md),
[ADR 0058](0058-p3-closed-multi-arg-helper-lowering.md),
[ADR 0059](0059-p3-rec-hidden-captures.md)

## Context

ADR 0026 deliberately kept Phase 1 codegen to closed lambdas and direct calls.
That restriction was useful while Tacit lacked type and effect information, but
Phase 4's higher-order work now depends on real function values:

- callbacks must be passed to `map`, `fold`, and `for-each`;
- closures must capture local values without rewriting programs into manual
  state threading;
- function values must be storable in records after Stage 2's product work;
- effect signatures must remain inspectable without row polymorphism or new
  effect atoms.

The canonical AST already has the source-level shape: `lam` constructs a
function and `app` applies one argument. Function types are already represented
by `fn-ty` with an `eff-set` or `eff-var` call-effect child. Stage 3 therefore
does not need a canonical-format amendment. It needs to lift the implementation
restriction while pinning capture, runtime, and effect rules tightly enough for
Stage 4.

## Decision

**Tacit function values are first-class closure values. A closure is a function
code pointer paired with an immutable environment pointer. Function application
remains unary at the source level; multi-argument functions are curried
function values, while codegen may preserve direct multi-argument calls for
known saturated lambda chains and `rec` members.**

No new canonical node kind is added. The existing forms keep their meaning:

```text
(lam body)        ; constructs a function value
(app fn arg)      ; applies one argument
(fn-ty a b eff)   ; function type, with call effect eff
```

### Runtime representation

A first-class function value lowers to a two-word closure pair:

| Field | Meaning |
| --- | --- |
| `code` | pointer to a compiler-generated closure-entry function |
| `env` | pointer to an immutable environment object, or null/static empty env |

For a closure of type `A -> B / E`, the closure-entry function has the logical
ABI:

```text
closure_entry(env*, arg: A) -> B
```

The concrete LLVM parameter and result types are derived from the static
`fn-ty` at the call site. The closure object does not carry runtime type,
arity, or effect metadata; those remain typechecker facts.

Environment objects are compiler-managed runtime data. They may be heap
allocated, stack promoted, statically allocated, or scalar-replaced as an
optimization, provided observable behavior is unchanged. The baseline Stage 4
implementation may use process-lifetime heap allocation for escaping
capturing closures. This does not introduce ownership, destructors, finalizers,
or user-visible allocation handles.

### Capture rules

Captures are by value. Because Tacit source values are immutable except for
region-limited handles, by-value capture is the only Phase 4 capture mode.
There is no by-reference capture syntax.

The captured environment is a minimized free-variable set:

- compute free DeBruijn references in the closure body after accounting for
  the lambda parameters introduced by the closure being built;
- include only referenced bindings that are outside the closure's parameter
  binders;
- order environment slots deterministically by the referenced DeBruijn index
  at the closure creation site, nearest binding first;
- capture each binding once even if it is referenced multiple times.

Only first-class, escapable values may be captured by first-class closures. At
Stage 4's required surface, this includes integers, booleans, records whose
fields are first-class, and function values. Other first-class value types
become capturable when they have a codegen representation. `Buf` and `I64Vec`
handles remain region-limited per ADR 0038 and ADR 0061: they may be used by
direct-call `rec` helpers through ADR 0059's hidden parameters, but they may
not be captured into a first-class closure.

If a later stage adds another region-limited or non-escapable type, it inherits
the same rule unless an ADR explicitly says otherwise.

### Direct-call preservation

Codegen must preserve existing direct-call behavior when the function target is
statically known and the application is saturated:

- closed lambda chains may still lower as private direct-call functions;
- known `let`-bound lambda chains may still call the private function directly;
- known `rec` members may still use ADR 0059 hidden capture parameters;
- a direct call may avoid constructing a closure object entirely.

These are optimizations of the closure semantics, not separate source-level
function kinds. The same program can be closure-converted or direct-called as
long as type, effect, result, and diagnostic behavior match.

When a known function is used in value position, passed as an argument,
returned, or stored in a record, codegen reifies it as a closure pair.

### First-class operations

Function values may be:

- passed as ordinary arguments;
- returned from functions;
- stored in records and projected back out;
- captured by other closures;
- applied through any expression whose static type is `fn-ty`.

Application is unary. A source expression of type `A -> B -> C` is a function
that accepts an `A` and returns a function `B -> C`. Applying fewer arguments
than a direct-call optimization expects is not an error once closures are
implemented; it produces a function value. Applying more arguments is parsed as
repeated unary `app` nodes and is valid only if each intermediate result is a
function.

There is no runtime function equality, hashing, serialization, or reflection in
Phase 4. A closure value can be stored and called, but not inspected.

### Effect rules

Function values carry effects in their static function type. For
`(fn-ty A B E)`, `E` is the effect of calling the function with one argument.
Evaluating a `lam` expression itself is pure at the source level: compiler
managed closure storage is not a source-visible `Alloc` effect. `Alloc` remains
the effect of source-level allocation primitives such as `@buf-alloc`,
`@buf-alloc-dyn`, and `@i64-alloc`.

The existing inference rule remains the source rule:

- `lam body` has evaluation effect `{}`;
- the function type's call effect is the inferred effect of `body`;
- `app f x` has evaluation effect `effect(f) union effect(x) union call_effect(f)`.

Captured functions contribute effects only when called in the closure body. If
a closure captures an effectful callback and merely returns or stores it, that
does not perform the callback's effect. If the body applies the callback, the
callback's call effect is joined into the closure's call effect by the ordinary
`app` rule.

No row polymorphism is introduced. A higher-order function may use one effect
variable as specified by ADR 0036, and all callback effects that flow through
that variable must unify with it. A wrapper that adds concrete effects cannot
express `{IO | e}` in Phase 4; it must use a concrete upper-bound effect set
or a less general monomorphic signature.

### `rec` and recursive closures

Known calls to `rec` members keep ADR 0059 direct-call lowering. If a `rec`
member is reified as a first-class function value, its closure environment is
the member's minimized outer capture set plus the recursive group identity
needed for self and mutual calls. The runtime representation is still a closure
pair; the group identity is an implementation detail, not a new source value.

Phase 4 does not add partial `rec` group values, open recursion records, or
user-visible recursive environment objects.

### Diagnostics

Stage 4 adds or sharpens these structured diagnostics:

| Kind | Producer | Meaning |
| --- | --- | --- |
| `apply-non-function` | typecheck | `app` function position has a non-function type. |
| `invalid-capture` | typecheck or codegen | closure capture set includes a non-capturable value such as `Buf` or `I64Vec`. |
| `unsupported-closure-escape` | codegen | implementation cannot safely reify or move a closure at the requested escape site. |
| `effect-violation` | typecheck | inferred call effect is not a subset of the declared `fn-ty` effect. Existing ADR 0041 kind, reused. |

`CodegenError::FirstClassFunction` and `CodegenError::FreeVarInLambda` cease to
be expected failures for well-typed Stage 4 programs. They may remain as
internal errors or compatibility aliases during migration, but the user-facing
diagnostics should move to the kinds above.

## Alternatives considered

- **Whole visible environment capture.** Rejected. ADR 0059 used whole
  visible captures for direct `rec` calls because unused hidden parameters are
  harmless there. First-class closures would accidentally capture
  non-escapable handles and bloat stored function values. Minimized capture
  sets are the right baseline for an ABI that can escape.
- **By-reference capture.** Rejected. Tacit-Lite has no user-visible mutable
  local variables, and region-limited handles already have explicit
  anti-escape rules. By-reference capture would force lifetime and aliasing
  machinery into Phase 4.
- **Function pointers without environments.** Rejected. ADR 0026 already
  rejected this as a dead-end partial ABI. Capturing closures are the Phase 4
  requirement.
- **Make every closure creation carry `Alloc`.** Rejected. Compiler-managed
  closure storage is not a user-visible allocation handle, and marking pure
  higher-order programs as allocation-effectful would make common callback
  idioms harder to annotate and inspect. Source-level allocation primitives
  remain effect-tracked.
- **Add a new canonical `closure` node.** Rejected. `lam` already expresses
  the source construct and `fn-ty` already expresses the type/effect boundary.
  Closure conversion is a lowering pass, not a canonical syntax feature.
- **Row-polymorphic captured effects.** Rejected. Phase 4 remains within
  ADR 0036's one-effect-variable model. Row polymorphism is explicitly Phase 7
  scope.

## Consequences

- ADR 0026's closed-lambda and no-first-class-function restrictions are lifted
  for Phase 4 Stage 4, but existing direct-call programs remain valid.
- No canonical-format amendment is required.
- Type inference can mostly keep its current function model: `Ty::Fn` remains
  the source of function-value type and call-effect information.
- Codegen must add closure conversion, closure-pair value lowering, indirect
  calls through typed closure entries, and environment construction.
- Record codegen must treat closure pairs as valid field values once Stage 4
  adds the closure value representation.
- `Buf` and `I64Vec` remain non-escapable. Higher-order programs that need
  mutation through those handles should pass the handle as an explicit
  argument to a direct helper or wait for a later lifetime/capability design.
- Higher-order combinators in Stage 5 can be typed with ADR 0036-style effect
  variables for callback effects, but combinators that add their own concrete
  effects will be less general until row polymorphism exists.

## Related decisions

- [ADR 0026](0026-phase-1-closed-lambdas.md) - restriction being lifted.
- [ADR 0034](0034-p2-type-subset-ann.md) - `fn-ty` and `forall`.
- [ADR 0035](0035-p2-effect-set-canonical.md) - fixed effect lattice.
- [ADR 0036](0036-p2-effect-polymorphism-syntax.md) - one effect variable.
- [ADR 0038](0038-p2-writable-buffer.md) - buffer anti-escape rule.
- [ADR 0041](0041-p2-structured-error-format.md) - structured diagnostics.
- [ADR 0058](0058-p3-closed-multi-arg-helper-lowering.md) - direct-call arity.
- [ADR 0059](0059-p3-rec-hidden-captures.md) - direct `rec` hidden captures.
- [ADR 0061](0061-p3-stdlib-bundle-a-i64-vectors.md) - `I64Vec` anti-escape.
- [ADR 0072](0072-p4-record-products.md) - function values can become record
  fields after this ADR's closure ABI lands.
