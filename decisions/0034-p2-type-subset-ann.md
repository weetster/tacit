# 0034 — Phase 2 type subset for `ann`: canonical amendment

**Status:** Accepted
**Date:** 2026-04-27
**Phase:** 2, Stage 1
**Closes:** [phase-2-plan.md Q-P2-1](../plans/phase-2-plan.md); [canonical-text-format.md § 11](../plans/canonical-text-format.md#11-open-items) open item "Type syntax inside `ann`"
**Amends:** [ADR 0013](0013-canonical-text-format-frozen.md) (canonical text format) — additive extension

## Context

The `ann` node (arity 2: expression, type-as-expression) has existed in the
canonical format since the Stage 2 freeze ([ADR 0013](0013-canonical-text-format-frozen.md)).
Its note column reads "Type syntax reuses expression kinds." An open item in
[canonical-text-format.md § 11](../plans/canonical-text-format.md) deferred
enumerating which expression kinds are valid in type position until
Stage 4's corpus exercised typed programs. Test vector V29 was reserved but
left with no canonical-bytes commitment.

Phase 2 now owns this decision. The Phase 2 typechecker
(`tacit-typecheck`) needs to know exactly which canonical shapes it will
encounter in type position, so it can validate annotations rather than
accept arbitrary expression subtrees. This ADR enumerates the valid type
expression forms and introduces the minimum new canonical node kinds required
to express function types and universally quantified generic types.

Three design constraints bound the choice:

1. **The canonical text format is frozen for existing tags.** New tags
   are permitted by the "additive evolution only" rule; no existing tag
   may be re-purposed ([ADR 0013 § Decision](0013-canonical-text-format-frozen.md)).
2. **Type syntax reuses expression kinds where possible.** The `ann`
   node's original note establishes this direction; the goal is to
   extend, not replace, existing node kinds.
3. **Phase 2 scope: basic generics only.** Higher-kinded types, type
   classes, dependent types, and refinement types are Tacit-Full and
   must not be anticipated here. One `forall` binder per type
   signature is the ceiling for Phase 2.

The existing test vector V11 (`(ann (int 5) (record x (sym Int) y (sym Int)))`)
already demonstrates that `record` and `sym` work in type position. What is
missing is a representation for function types, type variables, effect-set
annotations on function types, and universal quantification.

## Decision

**The valid type expression forms in the second child of `ann` are
enumerated below. Three new canonical node kinds are introduced: `fn-ty`,
`ty-var`, and `forall`. Effect-set representation (`eff-set`, `eff-var`)
is specified by [ADR 0035](0035-p2-effect-set-canonical.md) and
[ADR 0036](0036-p2-effect-polymorphism-syntax.md) respectively; this ADR
only constrains where the effect child of `fn-ty` may appear.**

### New canonical node kinds

The following rows are **appended** to the frozen node-kinds table in
[canonical-text-format.md § 2](../plans/canonical-text-format.md):

| Tag      | Arity    | Children                          | Notes                                               |
|----------|----------|-----------------------------------|-----------------------------------------------------|
| `fn-ty`  | 3        | arg-type, ret-type, eff-node      | Function type. eff-node must be `eff-set` or `eff-var`. |
| `ty-var` | 1        | decimal int                       | Type variable reference. Index is DeBruijn over enclosing `forall` binders. |
| `forall` | 3        | ty-count (int), eff-count (int), body | Universal quantification. ty-count ≥ 1, eff-count ≥ 0. body is a type expression. |

Tags are 4–7 ASCII bytes; consistent with the 3–8 byte constraint in § 2.

### Valid type expressions

The second child of `(ann expr TYPE)` must be one of:

1. **`(sym T)`** — base type name. `T` is a bare symbol; the typechecker
   validates that `T` is a known type name (`Int`, `Bool`, `Str`, etc.).
   The canonicalizer accepts any symbol; validity is a typecheck concern.

2. **`(ty-var N)`** — type variable reference. `N` is a non-negative
   decimal integer. DeBruijn indexing: `(ty-var 0)` is the variable
   introduced by the innermost enclosing `forall` (at position 0 of its
   binding list), `(ty-var 1)` is the next outward, etc. Type variables
   and value variables (`var`) have separate index spaces; `(ty-var 0)`
   and `(var 0)` are unrelated.

3. **`(record field₀ type₀ field₁ type₁ …)`** — record type. Reuses the
   existing `record` tag. Field pairs are sorted by field-symbol bytes per
   [ADR 0008](0008-record-field-ordering.md). Each `typeᵢ` is recursively
   a type expression.

4. **`(fn-ty arg-type ret-type eff-node)`** — function type. `arg-type`
   and `ret-type` are type expressions. `eff-node` must be `(eff-set …)`
   or `(eff-var N)` as defined by ADRs 0035 and 0036. The pure function
   `Int → Int` is `(fn-ty (sym Int) (sym Int) (eff-set))`.

5. **`(app type-fn type-arg)`** — type application. Reuses the existing
   `app` tag. `(app (sym List) (sym Int))` is the type `List Int`. Both
   children are type expressions.

6. **`(forall TY-COUNT EFF-COUNT body)`** — universally quantified type.
   `TY-COUNT` and `EFF-COUNT` are decimal integer literals (not
   s-expressions). `TY-COUNT ≥ 1`, `EFF-COUNT ≥ 0`. In `body`, `(ty-var K)`
   is valid for K ∈ 0..TY-COUNT−1 and `(eff-var K)` is valid for
   K ∈ 0..EFF-COUNT−1. DeBruijn convention for nested `forall`: the
   innermost binder's variables have the lowest indices (same convention
   as `lam`, `let`, `rec`).

7. **`(eff-set …)`** and **`(eff-var N)`** — defined by ADRs 0035 and
   0036; valid in type position only as the third child of `fn-ty`.

### Constraint: type-position validity is a typecheck concern

The canonical parser does **not** enforce that the type child of `ann`
uses only valid type expression kinds. A syntactically valid canonical
tree with `(ann (int 5) (lam (var 0)))` parses and hashes; the
typechecker rejects it with a structured diagnostic per
[ADR 0041](0041-p2-structured-error-format.md). This preserves the
canonical format's long-standing property that parsing does not carry
semantic constraints.

### DeBruijn detail for `forall`

`(forall N M body)` introduces N type variables simultaneously (at
positions 0…N−1) and M effect variables simultaneously (at positions
0…M−1). Within `body`:
- `(ty-var 0)` refers to position 0 of the type variable list (the
  "first" type variable in left-to-right authoring order).
- `(ty-var 1)` refers to position 1, etc.
- Effect variables indexed with `eff-var` follow the same convention in
  their own space.

When `forall` nodes nest:
```
(forall 1 0
  (forall 1 0
    (fn-ty (ty-var 0) (ty-var 1) (eff-set))))
```
In the inner body, `(ty-var 0)` is the inner `forall`'s variable and
`(ty-var 1)` is the outer's. Identical to how `lam` nested under `lam`
shifts DeBruijn indices.

### Test vectors shipped with this ADR

**V29 — generic identity function** (`29-ann-generic-id.canonical`):
```
(ann (lam (var 0)) (forall 1 0 (fn-ty (ty-var 0) (ty-var 0) (eff-set))))
```
Represents `id :: ∀a. a → a`. Checks that `forall`, `fn-ty`, `ty-var`,
and `eff-set` round-trip through the extended parser.

**V30 — monomorphic IO-annotated function** (`30-ann-io-fn.canonical`):
```
(ann (lam (var 0)) (fn-ty (sym Int) (sym Int) (eff-set IO)))
```
Represents `f :: Int → Int / IO`. Checks that `fn-ty` with a concrete
effect annotation (`eff-set IO`) round-trips. V30 depends on ADR 0035's
`eff-set` tag; both ADRs must be accepted before the vector is valid.

These vectors require the parser to be extended with the new tags (Stage 2
work). Stage 1 commits the vector bytes; Stage 2 makes them pass.

## Alternatives considered

- **Re-encode function types as records: `(record -> (sym Int) ret (sym Int))`.** 
  Avoids a new tag but creates an ambiguity: is a `record` in type position
  a record *type* or a function-type-encoded-as-record? The reader can't
  distinguish syntactically; the typechecker would need a "magic field name"
  convention. Rejected as unreadable and fragile.

- **Use `lam` for type-level functions and `app` for everything.** Canonical
  text already defines `lam` as "introduces 1 value-level name; body sees
  `(var 0)`." Re-purposing it for type-level abstraction violates the
  frozen tag semantics. A `(lam ...)` in type position would be ambiguous
  with a value-level lambda stored as annotation (unusual but not forbidden).
  Rejected.

- **Use `ctor` for function type: `(ctor -> arg ret)`.** `ctor` is already
  the data-constructor application node; using it for the function-type
  constructor would require the typechecker to distinguish "type-position
  ctor" from "value-position ctor" by context alone. Rejected for the same
  reason as the record encoding.

- **Separate type-level `app` (`ty-app`).** Would remove the dual-use of
  `app` but adds another tag. The "types reuse expression kinds" principle
  from the original `ann` note argues against this. `app` in type position
  is unambiguous given that the typechecker already knows it is in type
  position (from the `ann` child position). Accepted via the principle;
  `ty-app` not introduced.

- **Implicit universal quantification (no `forall` tag).** ML-style: any
  `ty-var` appearing free in a type expression is implicitly universally
  quantified at the outermost `ann`. Avoids the `forall` tag but makes the
  scope of type variables implicit, complicating the canonicalizer's job
  (it must infer the quantifier count to produce a stable hash) and making
  nested generic types impossible to represent. Rejected: explicit `forall`
  keeps quantification scope unambiguous in canonical bytes.

- **`forall` with explicit binding-list children instead of counts.** Like
  `rec`, which has N binding RHS children, `forall` could have N children
  representing each type variable's kind annotation. In Phase 2 all type
  variables have kind `*`, so each binding child would be a placeholder
  (e.g., `(sym *)` or a hole). This adds useless children for the common
  case while providing no information. Explicit integer counts are more
  compact and don't require a kind syntax (kind syntax is Tacit-Full scope).
  Accepted: integer counts.

## Consequences

- The canonical-text-format.md § 2 node-kinds table gains three rows
  (`fn-ty`, `ty-var`, `forall`). The § 11 open item "Type syntax inside
  `ann`" is resolved and removed.
- Test vector V29 (previously blocked) and V30 are committed to
  [`plans/test-vectors/`](../plans/test-vectors/).
- The canonical parser (`tacit-canonical::parse`) must be extended in
  Stage 2 to recognize and parse the three new tags. Until then, canonical
  text containing them hard-fails with "unknown tag" — the Phase 1 behavior
  for any unrecognized tag.
- The canonical emitter (`tacit-canonical::emit`) must emit the three new
  tags in Stage 2 when the typechecker produces typed ASTs. Phase 1 programs
  not using these tags are unaffected.
- The BLAKE3 hasher already handles arbitrary node kinds; no hash-layer
  changes are needed.
- The inspection-view renderer in `tacit-views` must render the new tags
  in Stage 5 (when `--types` flag is honored). Rendering policy is specified
  by the inspection-view ADRs; this ADR does not mandate a specific textual
  representation for the human-readable inspection view.
- `(ann expr type)` with a malformed type subtree (e.g., `(ann (int 5) (int 0))`)
  still parses and hashes. The typechecker produces a `type-parse-error`
  diagnostic; codegen is gated on a clean typecheck result.

## Related decisions

- [ADR 0013](0013-canonical-text-format-frozen.md) — this ADR is the first
  additive extension to the frozen canonical format.
- [ADR 0008](0008-record-field-ordering.md) — record field sorting applies
  inside record types, unchanged.
- [ADR 0035](0035-p2-effect-set-canonical.md) — `eff-set` tag (third child
  of `fn-ty`).
- [ADR 0036](0036-p2-effect-polymorphism-syntax.md) — `eff-var` tag.
- [ADR 0041](0041-p2-structured-error-format.md) — diagnostics emitted when
  type position contains an invalid expression kind.
- [phase-2-plan.md Q-P2-1](../plans/phase-2-plan.md) — closed by this ADR.
- [canonical-text-format.md § 11](../plans/canonical-text-format.md) —
  open item resolved; § 11 updated concurrently with this ADR.
