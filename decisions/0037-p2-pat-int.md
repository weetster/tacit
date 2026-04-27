# 0037 — Phase 2 `pat-int`: integer literal pattern canonical extension

**Status:** Accepted
**Date:** 2026-04-27
**Phase:** 2, Stage 1
**Closes:** [phase-2-plan.md Q-P2-4](../plans/phase-2-plan.md); [ADR 0032 § 3](0032-stage-4-frozen.md) smoke #7 deferral
**Amends:** [ADR 0013](0013-canonical-text-format-frozen.md) — additive extension

## Context

[ADR 0032 § 3](0032-stage-4-frozen.md) deferred smoke #7 (`match-int.tac`)
because it requires pattern-matching on integer literals — a capability not
present in the Phase 1 canonical pattern set (`pat-wild`, `pat-var`, `pat-ctor`).

The existing `match` / `arm` machinery is:
```
(match scrutinee (arm pat body) …)
```
where `pat` is one of `pat-wild`, `(pat-var)`, or `(pat-ctor name sub-pat…)`.
None of these covers "the scrutinee equals the integer literal N."

The canonical text format § 8 describes patterns but does not include an
integer-literal pattern. ADR 0032 noted this as a `pat-int` canonical
extension required for Phase 2. This ADR provides that extension.

The design is deliberately narrow: one new tag, one integer-literal child,
no value bindings, no range or guard syntax. Range patterns, bitfield
patterns, and string patterns are Tacit-Full scope and must not be
anticipated here.

## Decision

**A new canonical pattern kind `pat-int` matches when the scrutinee value
equals an integer literal. It introduces no binders.**

### New canonical node kind

Appended to canonical-text-format.md § 2 (pattern rows section) and § 8
(Patterns section):

| Tag       | Arity | Children      | Notes                                                             |
|-----------|-------|---------------|-------------------------------------------------------------------|
| `pat-int` | 1     | decimal int   | Integer literal pattern. Matches if scrutinee = literal. No binder. Same grammar as `(int N)` — negative integers permitted. |

### Semantics

- **Matching**: `(arm (pat-int N) body)` matches when the scrutinee
  evaluates to the integer N. The arm body sees no additional binding from
  `pat-int` (there is nothing to bind).
- **Scope**: the arm body's DeBruijn scope is unchanged relative to the
  scrutinee's scope — exactly as for `pat-wild`. `pat-int` does not
  introduce a new name.
- **Arm ordering**: preserved as written, first-match-wins, per the
  canonical format's existing `match` semantics. Multiple arms with the
  same `pat-int` literal are syntactically permitted; the canonicalizer
  does not deduplicate them (the first matching arm wins at runtime).
- **Integer grammar**: the literal child follows the same rules as
  `(int N)` in canonical text — decimal ASCII, no leading zeros except `0`
  itself, negative integers have a leading `-`, `-0` normalizes to `0`
  (per [ADR 0010](0010-canonical-emission-rules.md)).
- **Type at typecheck time**: the scrutinee must have type `Int`
  (or whatever numeric type `pat-int` is used with). The typechecker
  enforces that `pat-int N` is only used in arms where the scrutinee
  type is numeric. Phase 2 only has `Int` (i64); this is trivially
  satisfied for any `match` on an `Int`-typed scrutinee.

### Smoke #7: `match-int.tac`

The `match-int` smoke program demonstrates integer literal pattern matching.
Canonical form (simplified):
```
(match (int 0)
  (arm (pat-int 0) (int 42))
  (arm pat-wild    (int 0)))
```
This matches `0` and returns `42`. Stage 4 of Phase 2 adds `match-int.tac`
to the nine-program corpus and wires codegen for `pat-int`.

### Codegen for `pat-int` (Stage 4 implementation note)

Codegen lowers `(match scrutinee (arm (pat-int N) body) …)` as an integer
equality check: `icmp eq scrutinee, N`. If equal, branch to the arm's body
block; if not equal, branch to the next arm. The lowering follows the
existing `match` pattern-dispatch structure in
[ADR 0032](0032-stage-4-frozen.md)'s codegen path, adding one new arm
dispatch case for `pat-int`.

Wildcard and constructor arms continue to use their existing lowering.
The ordering of mixed `pat-int` / `pat-ctor` / `pat-wild` arms is
preserved; codegen emits linear equality-check cascades in arm order.

### Test vector shipped with this ADR

**V32 — `pat-int` in a `match` arm** (`32-pat-int-match.canonical`):
```
(match (int 5) (arm (pat-int 5) (int 1)) (arm pat-wild (int 0)))
```
Represents `match 5 { 5 -> 1; _ -> 0 }`. Checks that `pat-int`
round-trips through the extended canonical parser and is correctly handled
by the pattern-aware consumers.

## Alternatives considered

- **Encode integer patterns as `pat-ctor` with a name convention:
  `(pat-ctor 5)` or `(pat-ctor int-5)`.** `pat-ctor` is a constructor
  pattern; overloading it for integer literals conflates two distinct
  pattern semantics. A reader seeing `(pat-ctor 5)` would not know if
  `5` is a constructor name or a literal. Rejected.

- **Guard expressions: `(arm pat-wild (if (eq scrutinee N) body …))`.** 
  Rewrite integer patterns as wildcard + guard. This would work for the
  smoke corpus but produces non-canonical representations for what is
  semantically a pattern match on a literal. The canonical form for
  `case 5 of 5 -> ...` and `case 5 of _ | (5 == 5) -> ...` would
  differ even though they mean the same thing. Rejected: pattern-literal
  matching is semantically distinct from guard-based matching and should
  be canonical.

- **Add `pat-str` and `pat-sym` at the same time.** Phase 2 smoke #7
  only needs integer patterns. String and symbol patterns are Tacit-Full
  or at minimum deferred. Adding them now without a concrete consumer
  would repeat the Phase 1 mistake of adding `Hole` recovery without
  a consumer to drive the design. Rejected: one tag, one consumer, one
  ADR.

- **Range patterns: `(pat-int-range lo hi)`.** No Phase 2 program requires
  range patterns. Rejected.

## Consequences

- canonical-text-format.md § 2 and § 8 gain one row (`pat-int`). The
  Phase 1 pattern kind set (`pat-wild`, `pat-var`, `pat-ctor`) is now
  extended with `pat-int`.
- The canonical parser gains one new tag in Stage 2.
- The authoring-view parser gains the surface syntax `N` (an integer
  literal in pattern position) in Stage 4, lowered to `(pat-int N)`.
- The inspection-view renderer gains a `pat-int` case; it renders as
  the integer literal followed by nothing (no binding name).
- Smoke #7 (`match-int.tac`) is unblocked. Stage 4 of Phase 2 adds it
  to the corpus and CI.
- Codegen must be extended in Stage 4 to lower `pat-int` arms as integer
  equality checks. All other arm kinds are unchanged.

## Related decisions

- [ADR 0010](0010-canonical-emission-rules.md) — integer emission rules;
  `pat-int`'s literal child follows the same rules as `int`.
- [ADR 0013](0013-canonical-text-format-frozen.md) — amended (additive).
- [ADR 0032 § 3](0032-stage-4-frozen.md) — smoke #7 deferral closed
  by this ADR's canonical extension and Stage 4's implementation.
- [phase-2-plan.md Q-P2-4](../plans/phase-2-plan.md) — closed.
