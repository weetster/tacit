# 0036 — Phase 2 effect polymorphism surface syntax

**Status:** Accepted
**Date:** 2026-04-27
**Phase:** 2, Stage 1
**Closes:** [phase-2-plan.md Q-P2-3](../plans/phase-2-plan.md); parent plan Q2
**Amends:** [ADR 0013](0013-canonical-text-format-frozen.md) — additive extension

## Context

[docs/effect-system.md](../docs/effect-system.md) describes "basic effect
polymorphism" as the critical piece for higher-order functions: `map` cannot
require its callback to be pure if the user wants to map an IO-producing
function. The solution is one effect *variable* per higher-order function
that stands for whatever effect set the callback carries.

[phase-2-plan.md Q-P2-3](../plans/phase-2-plan.md) asks how effect
variables appear in the canonical, authoring, and inspection forms. The
parent plan's commitment:

> "Basic" means one effect variable per function; row polymorphism is
> Tacit-Full and out of scope.

[ADR 0035](0035-p2-effect-set-canonical.md) defines `eff-set` for concrete
effect sets. [ADR 0034](0034-p2-type-subset-ann.md) defines `forall N M body`
as the universal quantification form where N type variables and M effect
variables are introduced. This ADR fills in the `eff-var` reference node
and specifies how effect variables are written in authoring and inspection
views.

## Decision

**A new canonical node kind `eff-var` references an effect variable by
DeBruijn index. Effect variables are bound by `forall` nodes
(per ADR 0034). Phase 2 enforces M ≤ 1 per `forall` node.**

### New canonical node kind

Appended to canonical-text-format.md § 2:

| Tag      | Arity | Children      | Notes                                                             |
|----------|-------|---------------|-------------------------------------------------------------------|
| `eff-var` | 1    | decimal int   | Effect variable reference. DeBruijn index over `forall` eff-var binders. |

### DeBruijn indexing for effect variables

`(forall N M body)` introduces M effect variables simultaneously at
positions 0…M−1. Within `body`, `(eff-var 0)` refers to position 0's
effect variable, `(eff-var 1)` to position 1's, etc. Since Phase 2
requires M ≤ 1, `(eff-var 0)` is the only index that appears in
well-formed Phase 2 types.

When `forall` nodes nest, the DeBruijn convention is the same as for
`ty-var` (ADR 0034 § DeBruijn detail): the innermost binder's effect
variables have the lowest indices.

Effect variable indices are in a separate space from type variable indices.
`(eff-var 0)` and `(ty-var 0)` are different things even within the same
`forall`; the tag distinguishes them.

### Where `eff-var` appears

`eff-var` is valid **only** in the effect position (third child) of
`fn-ty`. It is not valid as the first or second child of `fn-ty`, in
value position, or anywhere outside a type expression. The typechecker
rejects out-of-position `eff-var`; the parser accepts it (same policy as
other type-only nodes per ADR 0034).

### Phase 2 constraint: M ≤ 1 per `forall`

Phase 2's effect polymorphism is "basic": at most one effect variable per
quantification context. A `(forall N M body)` with M > 1 is a scope
violation in Phase 2 — the typechecker rejects it with a structured
diagnostic. This constraint is not enforced by the parser; it is a Phase 2
typechecker policy. Phase 3+ may raise the limit without a canonical-format
change.

Row polymorphism (composing multiple effect variables with set-row
concatenation) is explicitly Tacit-Full scope and must not be implemented
in Phase 2. Any design pressure to use M > 1 for a Phase 2 program is a
signal that the program is Tacit-Full shaped; defer and simplify.

### Authoring view surface

In the authoring view, effect variables are written as single lowercase
letters following a slash after the return type:

```
map : (a -> b / e) -> List a -> List b / e
```

The authoring parser infers:
- `a`, `b` are type variables — bound by the implicit `forall` over type
  variables. The authoring projection resolves names to DeBruijn indices.
- `e` is an effect variable — bound by the implicit `forall`'s M=1 slot.

The canonical form of this signature (for authoring-view reference):
```
(forall 2 1
  (fn-ty
    (fn-ty (ty-var 0) (ty-var 1) (eff-var 0))
    (fn-ty (app (sym List) (ty-var 0))
           (fn-ty (app (sym List) (ty-var 1)) ... (eff-var 0))
           (eff-var 0))
    (eff-var 0)))
```

(The `List` result type is abbreviated here; the full form is two nested
`fn-ty` nodes since all functions are curried.)

In authoring view, type and effect variables share a single identifier
namespace. The authoring parser disambiguates: a lowercase identifier after
`/` is an effect variable; a lowercase identifier in any other type position
is a type variable. The same lowercase letter may not denote both a type
variable and an effect variable in the same signature — that is an authoring
parse error.

### Inspection view rendering

The inspection view with `--effects` honors the effect annotation on
`fn-ty` nodes. Effect variables render as a single lowercase letter
(the sidecar may record the original authoring name; otherwise the
inspection view assigns a canonical letter: the first effect variable is
`e`, the second is `f`, etc.). The dense (authoring-style) rendering uses
`/ e`; the verbose (inspection-style) rendering spells it out as
`effects: { e }` on a separate line.

### Test vector shipped with this ADR

**V31 — effect-polymorphic identity function** (`31-ann-eff-poly.canonical`):
```
(ann (lam (var 0)) (forall 1 1 (fn-ty (ty-var 0) (ty-var 0) (eff-var 0))))
```
Represents `id :: ∀(a, e). a → a / e` — an identity function that
transparently propagates its caller's effects. Checks that `forall` with
M=1, `fn-ty`, `ty-var`, and `eff-var` round-trip through the extended
parser.

## Alternatives considered

- **Represent effect variables as `sym` nodes with a naming convention:
  `(sym e)`.** Avoids a new tag but makes it impossible to distinguish
  a type variable named `e` from an effect variable named `e` in
  canonical form without context. Canonical form is context-free; two nodes
  with identical bytes must have identical semantics. Rejected.

- **Inline the effect variable into `forall` arity and use `ty-var` for
  effect variables too.** `(forall 3 body)` where the first 2 `ty-var`
  indices are type variables and index 2 is the effect variable. The
  problem: `ty-var` and `eff-var` appear in different positions (`ty-var`
  in type-expression position, `eff-var` only in the effect slot of
  `fn-ty`). With a shared tag, the position-validity rule would have to
  be tracked by context rather than by tag. Rejected: distinct tags make
  the distinction explicit.

- **Allow M > 1 now; enforce M ≤ 1 at the type-checker level.** Technically
  already the case — the parser does not enforce M ≤ 1. The Phase 2
  typechecker imposes the constraint. The question was whether to make it
  a canonical-format constraint. Decision: keep it as a typechecker policy
  so Phase 3+ can relax it without a canonical-format amendment.

- **Row polymorphism in Phase 2.** Explicitly rejected per
  [phase-2-plan.md § Risks — Effect-system creep](../plans/phase-2-plan.md)
  and [docs/effect-system.md § Why we split along this seam](../docs/effect-system.md).

## Consequences

- canonical-text-format.md § 2 gains one row (`eff-var`). Total new tags
  from Phase 2 Stage 1: `fn-ty`, `ty-var`, `forall` (ADR 0034), `eff-set`
  (ADR 0035), `eff-var` (this ADR).
- The canonical parser is extended in Stage 2 to recognize `eff-var`.
- The typechecker enforces the M ≤ 1 constraint at Phase 2; it also
  validates that `eff-var N` is in scope (N < M for the enclosing
  `forall`).
- Programs using `map`, `filter`, `fold`, and similar higher-order
  combinators over effectful callbacks can be typed without spurious
  effect errors. This is the primary payoff of Phase 2 effect
  polymorphism.
- Row polymorphism remains deferred. A Phase 2 program that needs to
  *compose* two effectful higher-order functions in a way that requires
  distinct effect variables signals Tacit-Full scope.

## Related decisions

- [ADR 0013](0013-canonical-text-format-frozen.md) — amended (additive).
- [ADR 0034](0034-p2-type-subset-ann.md) — `forall N M body` syntax;
  defines how M effect variables are introduced.
- [ADR 0035](0035-p2-effect-set-canonical.md) — `eff-set`; the concrete
  alternative to `eff-var` in `fn-ty`'s effect slot.
- [docs/effect-system.md](../docs/effect-system.md) — Lite vs. Full
  boundary; the "basic polymorphism = one variable per function" framing.
- [phase-2-plan.md Q-P2-3](../plans/phase-2-plan.md) — closed by this ADR.
