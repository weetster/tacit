# 0035 — Phase 2 effect-set canonical syntax and lattice ordering

**Status:** Accepted
**Date:** 2026-04-27
**Phase:** 2, Stage 1
**Closes:** [phase-2-plan.md Q-P2-2](../plans/phase-2-plan.md)
**Amends:** [ADR 0013](0013-canonical-text-format-frozen.md) — additive extension

## Context

[docs/effect-system.md](../docs/effect-system.md) specifies four atomic
effects for Tacit-Lite: `IO`, `Alloc`, `Mut`, `Div`. Phase 2 adds a type
and effect checker; it needs a canonical representation for effect sets so
that:

1. Effect sets in type annotations are byte-deterministic (two identical
   effect sets produce identical canonical bytes → identical hashes).
2. The subsumption (`⊆`) and join (`∪`) rules are computable from the
   canonical bytes alone, without consulting a metadata store.
3. The canonical form integrates naturally with the `fn-ty` node introduced
   by [ADR 0034](0034-p2-type-subset-ann.md), specifically its third child.

[ADR 0008](0008-record-field-ordering.md) established the precedent that
canonical form sorts record fields by byte value to achieve
hash-equality-of-semantic-equality for records. Effect sets have the same
requirement: `{IO, Mut}` and `{Mut, IO}` are semantically identical, so
they must hash identically. The same sort discipline applies.

The Phase 1 `libc-effects.toml` ([ADR 0025](0025-phase-1-libc-surface.md))
represents effect sets as TOML arrays: `["IO"]`. That file is an external
data format read by the effect checker at compile time; it is not part of
the canonical AST. This ADR specifies the in-AST canonical representation.

## Decision

**A new canonical node kind `eff-set` represents a concrete effect set.
Atoms are sorted alphabetically. The empty `(eff-set)` is the pure (bottom)
effect. `(eff-set Alloc Div IO Mut)` is the top (most effectful) set.**

### New canonical node kind

Appended to canonical-text-format.md § 2:

| Tag       | Arity | Children                     | Notes                                               |
|-----------|-------|------------------------------|-----------------------------------------------------|
| `eff-set` | 0+N   | atom₀, atom₁, … (bare syms) | Effect set. Atoms drawn from {Alloc, Div, IO, Mut}, sorted alphabetically. N=0 permitted (pure). |

`eff-set` children are bare symbols, not s-expressions. The same lexical
rule as field names in `record` and `pat-ctor` applies: each child matches
`[A-Za-z_][A-Za-z0-9_-]*`.

### Atom set

The four atomic effects for Tacit-Lite, fixed for Phase 2:

| Atom    | Meaning                                            | Sort position |
|---------|----------------------------------------------------|---------------|
| `Alloc` | Allocates memory (heap or stack)                   | 0 (first)     |
| `Div`   | May diverge (does not terminate)                   | 1             |
| `IO`    | Crosses an OS boundary (file, network, stdin/stdout)| 2             |
| `Mut`   | Mutates state observable outside the function      | 3 (last)      |

No other atoms are valid in `eff-set` for Phase 2. Adding a fifth atom
requires a new ADR amending this one. User-defined atoms are Tacit-Full
scope and must not be implemented in Phase 2.

### Canonical sort order

Atoms within `eff-set` are sorted ascending by ASCII byte values of the
atom name string. Since all four atoms are uppercase ASCII, the sort is:
`A` (65) < `D` (68) < `I` (73) < `M` (77), giving the fixed ordering
**Alloc < Div < IO < Mut**. Implementations must sort before emitting;
parsers must reject (or re-sort) out-of-order atoms.

Any `eff-set` node whose atoms are not in this strict ascending order
is a canonicalization error — the parser may either reject it or re-sort
and accept it (both are conforming behaviors, because canonical text
already normalizes record fields; the precedent is ADR 0008). The
canonicalizer must always emit sorted atoms.

### Lattice semantics

- **Bottom**: `(eff-set)` — the pure effect set (no effects). Every
  function that is provably free of side effects carries this set.
- **Top**: `(eff-set Alloc Div IO Mut)` — the maximally effectful set.
- **Join (∪)**: The union of two effect sets. Computed as set-union of
  atoms, sorted and deduplicated. `{IO} ∪ {Mut} = {IO, Mut}` →
  `(eff-set IO Mut)`.
- **Subsumption (≤)**: `A ≤ B` (A is less effectful than B) iff every
  atom in A is also in B. A function annotated with `{IO}` may be used
  wherever `{IO, Mut}` is expected; the annotation is a claim that the
  function carries *at most* those effects.

Hash-equality of semantic-equality is guaranteed by the sort constraint:
two sets with the same atoms always produce the same sorted byte sequence
and therefore the same BLAKE3 hash.

### Where `eff-set` appears

`eff-set` is a type-level construct. It appears:

1. **As the third child of `fn-ty`** (mandatory): every `fn-ty` node
   carries an explicit effect annotation. Pure functions carry `(eff-set)`.
2. **Nowhere else in the canonical format** (for Phase 2). Effect sets do
   not appear as standalone expressions outside of type position. Using
   `eff-set` in value position produces a typecheck error, not a parse
   error (same policy as other type-only node kinds per ADR 0034).

### Consuming `libc-effects.toml` in Phase 2

`stdlib/libc-effects.toml` stores effect sets as TOML arrays:
`tacit_effect_set = ["IO"]`. The effect checker reads this file and maps
each array to the equivalent `eff-set` atom list. The mapping is
straightforward: each string in the array becomes a bare symbol; atoms are
sorted before constructing the canonical representation.

### Test vectors shipped with this ADR

**V30 — IO-annotated function type** (`30-ann-io-fn.canonical`) (shared
with ADR 0034):
```
(ann (lam (var 0)) (fn-ty (sym Int) (sym Int) (eff-set IO)))
```

**V31 — Effect-polymorphic identity** (`31-ann-eff-poly.canonical`)
(shared with ADR 0036):
```
(ann (lam (var 0)) (forall 1 1 (fn-ty (ty-var 0) (ty-var 0) (eff-var 0))))
```
The `(eff-var 0)` in this vector is defined by ADR 0036; V31 is committed
by both this ADR and ADR 0036 together.

## Alternatives considered

- **Reuse `record` for effect sets: `(record IO (int 0) Mut (int 0))`.** 
  Record fields are already sorted (ADR 0008), so this would give
  hash-equality. But the placeholder value `(int 0)` is arbitrary and
  misleading — effect atoms have no associated value. A reader seeing this
  record can't distinguish it from a structural record type without out-of-
  band knowledge of context. Rejected: `eff-set` is more self-documenting.

- **`eff-set` with s-expression children: `(eff-set (sym IO) (sym Mut))`.** 
  Effect atoms are bare symbols, not expression subtrees; wrapping them in
  `(sym ...)` adds 6 bytes per atom for no semantic gain. Bare symbol
  children are already used in `record` (field names) and `ctor` (name).
  Rejected as unnecessarily verbose.

- **`eff-set` with sorted atoms but no duplicate rejection.** Permitting
  `(eff-set IO IO)` would make the parser more permissive but complicate
  the canonicalizer (must deduplicate) and the hash semantics (hash of
  `(eff-set IO IO)` ≠ hash of `(eff-set IO)` even though they're
  semantically equal). Rejected: the canonicalizer must deduplicate AND
  sort before emitting; the parser may reject duplicates or accept-and-
  deduplicate (same choice as the sort-order policy above).

- **Separate top-level effect-set declarations.** Some effect systems
  (Koka, Frank) represent effect sets in a separate row/type context that
  tracks available effects. Phase 2's fixed-lattice design does not need
  this. Rejected.

- **A fixed-arity `eff-set` with four boolean children.** `(eff-set 0 1 0 0)`
  for `{Div}`. Positionally ordered; no sort needed. Rejected as unreadable
  and dependent on remembering the position convention.

## Consequences

- canonical-text-format.md § 2 gains one row (`eff-set`). No existing row
  changes.
- The canonical parser gains one new tag in Stage 2. The parser must
  validate atom membership (only {Alloc, Div, IO, Mut} for Phase 2) and
  reject out-of-order atoms (or re-sort, either is conforming).
- The typechecker (`tacit-typecheck`) reads `eff-set` nodes to determine
  a function's declared effect set, compares against the inferred set, and
  produces `effect-violation` diagnostics for mismatches.
- `libc-effects.toml` consumption: the effect checker reads the TOML
  file and maps each TOML `tacit_effect_set` array to the corresponding
  `eff-set` canonical form when constructing the primitive effect environment.
  The TOML file does not change; its schema (frozen by ADR 0025) is still
  TOML arrays of strings, not canonical S-expressions.
- The inspection view's `--effects` flag (reserved by
  [ADR 0015](0015-inspection-view-scope.md)) renders `eff-set` atoms as a
  set notation, e.g. `{IO, Mut}`, in Stage 5. This ADR does not mandate
  the exact rendering.

## Related decisions

- [ADR 0008](0008-record-field-ordering.md) — precedent for sort-for-hash
  discipline on semantically unordered sets.
- [ADR 0013](0013-canonical-text-format-frozen.md) — amended by this ADR
  (additive).
- [ADR 0025](0025-phase-1-libc-surface.md) — `libc-effects.toml` schema;
  consumed but not changed by the effect checker.
- [ADR 0034](0034-p2-type-subset-ann.md) — introduces `fn-ty`, whose third
  child must be `eff-set` or `eff-var`.
- [ADR 0036](0036-p2-effect-polymorphism-syntax.md) — `eff-var` tag; the
  alternative to `eff-set` in `fn-ty`'s effect position for polymorphic
  functions.
- [phase-2-plan.md Q-P2-2](../plans/phase-2-plan.md) — closed by this ADR.
