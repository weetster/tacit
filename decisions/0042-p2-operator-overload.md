# 0042 — Phase 2 operator overload resolution

**Status:** Accepted
**Date:** 2026-04-27
**Phase:** 2, Stage 1
**Closes:** [phase-2-plan.md Q-P2-9](../plans/phase-2-plan.md)

## Context

Phase 1 operates with a single numeric type: `i64` (all integers, no type
annotation required). [ADR 0030](0030-phase-1-arith-primitives.md)
introduced arithmetic (`@add`, `@sub`, `@mul`, `@div`, `@mod`) and
comparison (`@eq`, `@ne`, `@lt`, `@le`, `@gt`, `@ge`) operators as
`@name` primitives, all with a fixed `i64 × i64 → i64` signature (or
`i64 × i64 → i64` for comparisons, where `0`/`1` encode false/true).

Phase 2 introduces `Int` as the canonical name for the `i64` type
([ADR 0034](0034-p2-type-subset-ann.md)), and the phase-2-plan notes
that operator overload resolution becomes relevant when "numeric width
types are in scope." The parent plan's commitment on coercion is firm:

> Coercion is forbidden; this is a *resolution* discipline, not a coercion one.

This ADR specifies the resolution rules for Phase 2 and introduces
`Bool` as a base type returned by comparison operators. It also establishes
the policy for future numeric width types (deferred to Phase 3+) so that
the Phase 2 typechecker handles them predictably if a program uses them.

## Decision

**Operator resolution is local and symmetric: both operands must unify to
the same type, no coercion is performed, and the operator's return type
is determined by the resolved operand type (arithmetic) or is always `Bool`
(comparison).**

### Numeric type set for Phase 2

The only numeric type implemented in Phase 2 is:

| Authoring name | Canonical form   | LLVM type | Phase 1 mapping |
|----------------|------------------|-----------|-----------------|
| `Int`          | `(sym Int)`      | `i64`     | the Phase 1 "everything is i64" type, named |

Additional width types (`Int32`, `Int16`, `Int8`, `Int64`, `Nat`,
`Float64`, etc.) are **reserved names** in Phase 2: the typechecker
recognizes them as valid `sym` tokens but immediately produces
`unresolved-type` if they appear in a type annotation or an inferred
position, with a `message` noting they are reserved for Phase 3+. No
LLVM lowering exists for them in Phase 2.

### `Bool` type

`Bool` is introduced as a base type in Phase 2:

| Authoring name | Canonical form | LLVM type | Notes              |
|----------------|----------------|-----------|--------------------|
| `Bool`         | `(sym Bool)`   | `i1`      | Comparison result  |

`True` and `False` are constructors: `(ctor True)` and `(ctor False)` with
type `Bool`. These use the existing `ctor` canonical node; no new tag is
needed.

`if` accepts `Bool` in its condition position. For compatibility with
Phase 1's `Int`-truthiness semantics ([ADR 0030](0030-phase-1-arith-primitives.md)),
`if` also accepts `Int` in the condition position in Phase 2, lowering as
`icmp ne cond, 0`. The typechecker does not warn on `Int`-condition `if`
in Phase 2; this policy may change in Phase 3.

### Resolution rules for binary operators

For each operator application `(app (app (sym op) e₁) e₂)` where `op` is
in the ARITH or CMP set:

1. **Infer both operand types** using local inference within the function
   body. Call them T₁ (type of e₁) and T₂ (type of e₂).

2. **Unify T₁ and T₂.** The unification is structural: type variables are
   solved; concrete types must be equal. No widening, no coercion.
   - If T₁ = T₂ = `Int`: resolve to the `Int` operator. Return type:
     `Int` for ARITH, `Bool` for CMP.
   - If T₁ = T₂ = some type variable `a`: resolve as a generic operator
     over `a`. For ARITH, return type is `a`; for CMP, return type is
     `Bool`. The typechecker will later instantiate `a` when a concrete
     call site provides operand types.
   - If T₁ ≠ T₂ and neither is a variable: emit `operator-overload-failure`
     per [ADR 0041](0041-p2-structured-error-format.md) and treat the
     expression as having an unknown type (proceed without cascading).

3. **No dispatch table.** Phase 2 has one numeric type (`Int`) plus
   `Bool`. Overload resolution is trivial: there is one candidate for each
   operator/operand-type combination. The "overload" machinery exists to
   enforce the no-coercion rule and to produce structured diagnostics when
   operand types disagree.

### Operator signatures in Phase 2

| Category | Operators                          | Operand type(s) | Return type |
|----------|------------------------------------|-----------------|-------------|
| ARITH    | `add`, `sub`, `mul`, `div`, `mod` | `Int`, `Int`    | `Int`       |
| CMP      | `eq`, `ne`, `lt`, `le`, `gt`, `ge`| `Int`, `Int`    | `Bool`      |

These match the Phase 1 behavior with explicit `Int` and `Bool` types. The
LLVM lowering for ARITH (`add nsw`, `sub nsw`, `mul nsw`, `sdiv`, `srem`)
and CMP (`icmp` + `zext`) is unchanged from [ADR 0030](0030-phase-1-arith-primitives.md),
except that CMP now `zext`s to `i1` (Bool) rather than `i64`. Phase 1
programs that use the `0`/`1` integer encoding of booleans may need to
be updated in Phase 2; this is expected and acceptable.

### Effect of arithmetic and comparison

Unchanged from [ADR 0030](0030-phase-1-arith-primitives.md): both ARITH
and CMP operators carry effect set `{}` (pure compute). The typechecker
reads this from a built-in effect table (not from `libc-effects.toml`,
which is OS-boundary only per [ADR 0025](0025-phase-1-libc-surface.md)).

### Future width types (Phase 3+)

When Phase 3+ introduces width types, the overload resolution algorithm
extends with a candidate table: for each (operator, T₁, T₂) tuple, a
dispatch table maps to a concrete operator implementation. The `no-coercion`
rule remains: if T₁ ≠ T₂, no candidate matches and the error is
`operator-overload-failure`. This ADR's resolution structure is the
Phase 2 base case that Phase 3+ extends.

## Alternatives considered

- **Implicit `Int` coercions for narrower width types.** A common choice
  in C-family languages: smaller types silently widen in arithmetic
  expressions. Explicitly rejected by the parent plan ("coercion is
  forbidden"). The resolution discipline allows the type system to enforce
  programmer intent.

- **Unified numeric type (no `Int`, `Bool` distinction; use `i64`
  everywhere).** Phase 1's model. Rejected for Phase 2: the parent plan
  commits to a proper type system; `Bool` and `Int` being distinct types
  is fundamental to the typechecker's utility.

- **`Bool` as `Int` in the type system (no new type).** Keep using `0`/`1`
  as booleans. Rejected: comparison operators would return `Int`, and `if`
  would accept `Int` without warning, making the type system useless for
  catching the common error of using an `Int` where a comparison result is
  expected. Introducing `Bool` as a distinct type is the minimum for
  meaningful type checking.

- **`True`/`False` as `pat-ctor`-compatible constructors in `match`.** Yes:
  since `True` and `False` are `(ctor True)` and `(ctor False)`, they work
  with existing `pat-ctor` patterns. No new pattern kind is needed for
  `Bool` matching. This is consistent with the existing `ctor`/`pat-ctor`
  design.

- **Generic numeric type class `Num a`.** Would allow arithmetic over
  any type that implements `Num`. Phase 2 has no type classes; this is
  Tacit-Full scope. The constraint-free generics of Phase 2 (parametric
  type variables only) can still express `Num`-like patterns by specializing
  at the call site, but without compile-time enforcement.

## Consequences

- The typechecker (`tacit-typecheck`) gains an operator-resolution pass
  that applies the unification rules above to every ARITH/CMP application.
- Comparison operators now return `Bool` in Phase 2. Phase 1 programs
  that use comparison results as integers (e.g., `@add (@eq x 0) 1`) will
  produce a `type-mismatch` diagnostic in Phase 2. Such programs need to be
  rewritten using explicit `if` or `match`.
- The seven Phase 1 smoke programs gain explicit `Int`/`Bool` signatures
  as part of Stage 2's typechecking exercise.
- `if` in Phase 2 accepts both `Bool` (idiomatic) and `Int` (Phase 1
  compatibility). The dual acceptance is deprecated: Phase 3 should warn;
  Phase 4+ should remove `Int`-condition `if`.
- The reserved-type-name list (`Int32`, `Int16`, `Int8`, `Int64`, `Nat`,
  `Float64`) prevents user-defined types from squatting on names that Phase
  3+ will use. The typechecker produces `unresolved-type` for these names
  in Phase 2 with a message distinguishing "reserved, not yet implemented"
  from "truly unknown."

## Related decisions

- [ADR 0030](0030-phase-1-arith-primitives.md) — Phase 1 operator
  definitions; Phase 2 adds types but preserves LLVM lowering.
- [ADR 0034](0034-p2-type-subset-ann.md) — `(sym Int)`, `(sym Bool)`;
  these are the concrete types that appear in operator signatures.
- [ADR 0041](0041-p2-structured-error-format.md) — `operator-overload-failure`
  error kind.
- [phase-2-plan.md Q-P2-9](../plans/phase-2-plan.md) — closed.
