# 0008 — Record field ordering: sorted by field-symbol bytes

**Status:** Accepted
**Date:** 2026-04-21
**Phase:** 0, Stage 2

## Context

Record literals in Tacit are semantically unordered: `{a: 1, b: 2}` and `{b: 2, a: 1}` denote the same record. Content-addressing requires that semantically-equal ASTs hash equally, so canonical form must produce the same byte sequence for both spellings.

This is the *only* AST construct where canonical form overrides user-supplied order. Match arms (first-match-wins), `ctor` arguments (positional), and `rec`/`module` bindings (per [ADR 0007](0007-debruijn-rec-indexing.md)) all preserve user order because their semantics are order-sensitive or because reordering would require a hash-fixpoint computation.

## Decision

**Record fields in canonical form are sorted ascending by the byte sequence of the field-symbol (UTF-8 lexicographic comparison).** A `record` node's children are flattened pairs `sym₀ val₀ sym₁ val₁ …`; the pairs are ordered such that `sym₀ < sym₁ < …` byte-wise.

Field-symbol uniqueness is a parser concern — duplicate fields in a record literal are an error and produce a `(hole arity-mismatch …)` rather than a `record` node. Canonical form does not need to handle the duplicate case.

This rule applies identically to record types when they appear inside `ann` (since types reuse expression kinds per [canonical-text-format.md § 2](../plans/canonical-text-format.md#2-node-kinds)).

## Alternatives considered

- **Preserve user-supplied order.** Forfeits hash-equality for semantic-equality of records. Unacceptable for the primary structured data type — users would observe surprising hash differences for records that round-trip through different writers.
- **Sort by hash of field value.** Would give the same hash for `{a: 1, b: 2}` and `{a: 1, b: 2}` regardless of order, but `{a: 1, b: 1}` would have an undefined order between the two equal-hashed values. Sorting by field-symbol is sufficient and avoids the tie-breaking question.
- **Disallow user-supplied order entirely (require sorted input in the authoring view).** Hostile to authors. Rejected — the canonicalizer's job is to absorb authoring-view permissiveness; pushing it back on users defeats the point of the view abstraction.
- **Sort by Unicode code-point sequence after NFC normalization.** Avoids the case where two visually-identical field names compare differently due to combining-mark variations. Rejected for the same reasons as in [ADR 0006](0006-canonical-lexical-rules.md): adds a Unicode-version dependency to the hash domain, and identifier well-formedness is better enforced at the parser. Field symbols are restricted to ASCII per ADR 0006 anyway, so the question is academic for Phase 0.

## Consequences

- **Authoring-view writers can use any field order.** The canonicalizer reorders before emitting canonical text.
- **The inspection view (Stage 3) faces a separate decision** about whether to display fields in canonical (sorted) order or in the user's preferred order pulled from the sidecar. That choice does not affect this ADR.
- **Hash-by-content for records works as expected.** `{a: 1, b: 2}.hash == {b: 2, a: 1}.hash`, which is the property users naturally assume.
- **Sort comparison is byte-wise on UTF-8.** No locale-dependent collation, no Unicode-version sensitivity. Two implementations cannot disagree on the order.
- **Record types in `ann` get the same treatment.** A type like `{x: Int, y: Int}` canonicalizes the same way as the value-level record literal.
- **Phase 1 type-checker and runtime treat field order as semantically irrelevant**, consistent with this canonical convention. No correctness pressure to expose the canonical order to users.

## Related decisions

- [ADR 0005](0005-canonical-surface-form.md) — surface form for `record` nodes.
- [ADR 0006](0006-canonical-lexical-rules.md) — field-symbol lexical rules (ASCII-only for Phase 0).
- [ADR 0007](0007-debruijn-rec-indexing.md) — the contrasting case where canonical preserves user order.
