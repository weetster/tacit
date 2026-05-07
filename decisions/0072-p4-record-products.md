# 0072 — Phase 4 product types: records first, tuples deferred

**Status:** Accepted
**Date:** 2026-05-07
**Phase:** 4, Stage 1
**Closes:** [phase-4-plan.md Q-P4-1, Q-P4-2](../plans/phase-4-plan.md)
**Affirms:** [ADR 0008](0008-record-field-ordering.md), [ADR 0034](0034-p2-type-subset-ann.md)

## Context

Phase 3 identified multi-return values and accumulator threading as the
dominant remaining structural gap. ADR 0070 carried tuples / records forward
as the first Phase 4 design question.

The implementation already has most of a record surface:

- canonical `record` and `proj` nodes, with field sorting pinned by ADR 0008;
- authoring syntax `{field: expr, ...}` and projection syntax `expr.field`;
- sidecar `field_order` for preserving authoring order;
- structural record types in annotations per ADR 0034;
- type inference and unification for exact-shape record values.

The missing load-bearing piece is codegen. `tacit-codegen` currently rejects
`record` and `proj` in expression position. Adding tuples as a new canonical
construct before records work would duplicate an existing product-type
substrate and reopen canonical-format questions without evidence that records
are insufficient.

## Decision

**Phase 4 product types are records. Tuple syntax and tuple canonical nodes
are deferred.**

Records are the only first-class product type in Phase 4 Stage 2. They are
structural, named-field products using the existing canonical forms:

```tacit
{value: 1, next: 2}
r.value
{value: Int, next: Int}
```

Canonical:

```text
(record next (int 2) value (int 1))
(proj (var 0) value)
(record next (sym Int) value (sym Int))
```

Field order in canonical text is sorted by field-symbol bytes per ADR 0008.
Authoring order is sidecar metadata only.

### Type semantics

Record types are exact structural shapes:

- two record types are equal when they have the same field names and matching
  field types;
- field order is not semantically relevant;
- no nominal record declarations are introduced;
- no width subtyping is introduced in Phase 4;
- a projection `r.f` typechecks only when `r` has an exact record type that
  contains field `f`.

This matches the existing `tacit-typecheck` record unification model and keeps
Phase 4 away from row polymorphism.

Existing non-first-class rules still apply. In particular, buffer handles from
ADR 0038 remain region-limited and may not be stored in records unless a later
ADR explicitly changes the buffer escape rules.

### Runtime and codegen semantics

Stage 2 codegen lowers a record value to an aggregate with fields in canonical
sorted order. Projection lowers to extraction of the canonical field index.

The runtime layout is not user-observable. It must be deterministic for a
given canonical record type, but it does not become part of the canonical hash
domain. A later optimization may scalar-replace records, pass them in registers,
or store them on the stack as long as source-level behavior and diagnostics are
unchanged.

Records may be passed to and returned from Tacit functions once codegen
supports aggregate parameter and result types. Stage 2 only needs to support
field types already supported by codegen at that point; closures become valid
record fields after the closure ABI lands in later Phase 4 stages.

### Destructuring and patterns

Phase 4 does not add record patterns.

Destructuring is projection-based:

```tacit
let p = f n in
@add p.value p.next
```

Pattern matching can bind or ignore the whole record using existing patterns
(`pat-var`, `pat-wild`) and then project fields in the arm body. A dedicated
`pat-record` form is deferred because it would require a new canonical pattern
tag, binder-order rules for multiple fields, and more surface area than Stage 2
needs.

### Tuple policy

No tuple syntax lands in Phase 4 Stage 2. A later ADR may add tuple authoring
syntax if records do not satisfy the Phase 4 corpus and primer goals.

If tuple syntax is reconsidered, it must choose between:

- authoring sugar over records with reserved positional field names; or
- a new canonical tuple node with positional semantics.

That choice is intentionally deferred until records have been measured.

### Test-vector expectations

No new canonical-format test vector is required by this ADR because it does
not add canonical tags. Existing vectors already cover the canonical substrate:

- V5 / V6: record ordering and nested records;
- V10a: empty record;
- V11: record types in annotations;
- V12: nested projection.

Stage 2 must add behavioral smoke tests for:

- constructing a record and projecting each field;
- returning a record from a function and projecting from the caller;
- passing a record to a function;
- nested records;
- an invalid projection diagnostic;
- a record type mismatch diagnostic.

## Alternatives considered

- **Add tuples only.** Rejected. Tuples are compact for short multi-return
  values, but they do not use the existing canonical `record` / `proj`
  substrate and are weaker as a reasoning surface because field meaning is
  positional.
- **Add both tuples and records now.** Rejected. Records already exist in the
  canonical and typechecker layers. Adding tuple syntax before records are
  executable increases implementation and primer scope before we know records
  are insufficient.
- **Encode tuples as records with reserved fields such as `_0`, `_1`.**
  Rejected for Stage 2. It would reserve field names that are currently valid
  user fields and create two authoring forms for the same canonical shape.
- **Add a new canonical `tuple` node.** Rejected for Stage 2. This is an
  additive canonical-format extension with no current need; records solve the
  Phase 3 structural gap with less churn.
- **Add record patterns now.** Rejected. Projection-based destructuring is
  enough for multi-return and accumulator records. Record patterns can be added
  later if they prove load-bearing.
- **Add width subtyping or row polymorphism.** Rejected. Exact structural
  equality is already implemented and sufficient for Phase 4. Row polymorphism
  remains Phase 7 scope.

## Consequences

- Stage 2 is narrower than "invent product types": it makes existing records
  executable and production-quality.
- No canonical-format amendment is required for Stage 2.
- The primer can teach one product idiom: named-field records plus projection.
- Rust-relative density may improve less than tuple syntax would for tiny
  two-field returns, but records provide clearer reasoning support and lower
  implementation risk.
- If records fail to move Phase 4 examples or corpus references enough, the
  freeze ADR records that finding and a later ADR can add tuple syntax with
  evidence.

## Related decisions

- [ADR 0008](0008-record-field-ordering.md) — canonical field ordering.
- [ADR 0014](0014-sidecar-format.md) — `field_order` display metadata.
- [ADR 0034](0034-p2-type-subset-ann.md) — record types in annotations.
- [ADR 0038](0038-p2-writable-buffer.md) — buffer handles are not first-class
  record-storable values.
- [ADR 0070](0070-p3-frozen.md) — Phase 3 defers tuples / records to Phase 4.
