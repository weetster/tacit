# 0013 — Canonical text format frozen

**Status:** Accepted
**Date:** 2026-04-22
**Phase:** 0, Stage 2 (exit)
**Freezes:** [plans/canonical-text-format.md](../plans/canonical-text-format.md)

## Context

Phase 0 Stage 2 is the specification of a byte-exact textual form for the Tacit-Lite AST, gated on a single empirical criterion from [phase-0-plan.md § Stage 2](../plans/phase-0-plan.md):

> two independent implementations must produce identical bytes for the same AST.

All other Stage 2 work (spec prose, ADRs 0005–0012, the 45-file fixture set under [`plans/test-vectors/`](../plans/test-vectors/)) existed to support that criterion. The empirical check was outstanding until 2026-04-22.

Two canonicalizers were written against the spec and the fixture set:

- [`impls/py-canonicalizer/`](../impls/py-canonicalizer/) — Python reference (built first, landed earlier the same day).
- [`impls/rs-canonicalizer/`](../impls/rs-canonicalizer/) — Rust reference (built independently from the spec + fixtures, with no reference to the Python source during porting).

Both implementations are thin: lexer → s-expression reader → typed AST builder → emitter → BLAKE3 hash. Neither shares code or dependencies with the other beyond the `blake3` crate / `blake3` Python package (both of which are thin wrappers around the upstream reference implementation).

The cross-implementation check compared BLAKE3 hashes for all 38 `*.canonical` fixtures:

```
impls/rs-canonicalizer/target/debug/dump-hashes  plans/test-vectors/  > rust.txt
(python one-liner driving impls/py-canonicalizer)                     > py.txt
diff rust.txt py.txt      # empty
```

Both implementations also agreed on the disposition of every `*.forbidden` and `*.reject` fixture (structural and lexical hard errors respectively), and both pass the V8 (hole hash stability), V17 (rec order preservation), and V19 (match arm order preservation) property checks.

With the gate met, the spec document moves from Draft to Frozen.

## Decision

**The canonical text format is frozen as of 2026-04-22, at the state captured in [`plans/canonical-text-format.md`](../plans/canonical-text-format.md) and ADRs 0005–0012.**

Concretely:

- The document's `Status:` header changes from `Draft (Stage 2 in progress; freezes at Stage 2 exit)` to `Frozen 2026-04-22` with a link to this ADR.
- The node-kind table in § 2, the lexical rules in § 3, the DeBruijn conventions in §§ 4–5, the record-field-ordering rule in § 6, the hole-diag-id initial set in § 7, the pattern-kind set in § 8, and the hashing rule in § 9 are all locked.
- Further changes to any of the above require a new ADR. Per [CLAUDE.md](../CLAUDE.md)'s ground rules, such changes are treated as spec bugs, not scope negotiation.

**Three open items carry forward and are explicitly *not* part of the freeze:**

1. **Exact hole diag-id set (§ 7).** Additive only; Phase 1 may extend the table as the parser hardens. An additive change here is not a format change.
2. **Type-expression subset inside `ann` (§ 11).** Deferred to Stage 4 corpus-driven decision; blocks only V29 (absent from the fixture set).
3. **bpe-compact authoring-view corpus-shape recheck (§ 11, from [ADR 0003](0003-authoring-view-bpe-compact.md)).** Revisited at Stage 4 corpus freeze. If the lead reverses, ADR 0003 is superseded but this freeze is unaffected — the canonical format does not depend on the authoring view.

## Alternatives considered

- **Wait for a third implementation before freezing.** Rejected. Two agreeing implementations from the same spec, written without cross-reference, is what Stage 2's exit criterion asks for. Demanding a third raises the bar arbitrarily and has diminishing returns — the practical divergence-probing value sits overwhelmingly in the first independent port, which is what surfaces spec ambiguity.
- **Defer the freeze until V29 (type-subset inside `ann`) resolves.** Rejected. The `ann` node's structural shape is fixed by § 2 (two children: expression, type-as-expression). The V29 open item is about *which subset of expression kinds are valid in the type slot* — a semantic constraint enforceable at a later layer, not a change to canonical bytes. Coupling the freeze to it would push Stage 2 out by the length of Stage 4 corpus work.
- **Freeze only a subset of the spec (e.g. exclude `module` and `hole`).** Rejected. Both features have fixtures and passed independent-implementation testing; carving them out would complicate downstream references for no substantive reason. If Phase 1 surfaces a genuine problem with either, an ADR can narrow or retract — that's the escape hatch, and it applies equally to any other section.
- **Leave the spec at Draft indefinitely.** Rejected. Stages 3 (view grammars + Rust AST enum) and 4 (corpus freeze) branch off a frozen canonical format; leaving it Draft blocks both and leaves the exit criterion unsignaled.

## Consequences

- **Downstream phases unblock.** Stage 3 can derive a Rust AST enum from § 2 without worrying about re-edits. Phase 1's parser and content-addressing work can reference specific line items of the spec knowing they will not move under them.
- **Two reference implementations exist.** [`impls/py-canonicalizer/`](../impls/py-canonicalizer/) and [`impls/rs-canonicalizer/`](../impls/rs-canonicalizer/) are now the conformance artifact for any future canonicalizer. A third implementation (in any language) is expected to reproduce the same 38 hashes; divergence is a bug in the new implementation or a spec bug discovered late (and thus requires an ADR).
- **Fixture directory is load-bearing.** `plans/test-vectors/` is no longer a Stage 2 drafting artifact — it is part of the conformance surface. New fixtures may be added (additive, with an accompanying note or ADR if they exercise a boundary the spec did not previously pin); existing fixtures may not be retroactively edited without a spec-bug ADR.
- **"Two implementations agree" is not proof of spec completeness.** Any implementation-level convergence on behavior not clearly specified is a latent divergence risk for a third implementation. Future ADRs should continue to specify tightenings inline (e.g. ADR 0010 for emission rules, ADR 0012 for scalar-value restriction) rather than relying on convergent implementation choice.
- **Open items are explicitly scoped.** A reader who sees V29 still open, or the § 7 diag-id table marked initial, should not treat the freeze as weak — those items are deliberately outside the scope of what Stage 2 needed to pin.

## Related decisions

- [ADR 0005](0005-canonical-surface-form.md) through [ADR 0012](0012-unicode-scalar-value-restriction.md) — the content of the freeze.
- [ADR 0003](0003-authoring-view-bpe-compact.md) — authoring view; unaffected by the freeze (separate projection layer).
- [phase-0-plan.md § Stage 2](../plans/phase-0-plan.md) — exit criterion this ADR signals completion of.
