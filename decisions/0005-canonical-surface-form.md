# 0005 — Canonical surface form: s-expressions with keyword tags

**Status:** Accepted
**Date:** 2026-04-21
**Phase:** 0, Stage 2

## Context

Stage 2 must specify a byte-exact canonical text format for the Tacit-Lite AST. Several surface forms are available:

- S-expressions with short ASCII keyword tags (`(lam (var 0))`)
- S-expressions with integer kind IDs (`(1 (12 0))` with a separate kind-id table)
- JSON-style records (`{"kind": "lam", "body": {"kind": "var", "i": 0}}`)
- A custom binary format (length-prefixed kind tag bytes, varint indices)

The canonical form is read by humans during Phase 1 implementation and debugging, parsed by two independent implementations that must agree byte-exactly, hashed by BLAKE3, and stored as the source of truth for content-addressing. The choice trades off readability, parser simplicity, byte size, and spec surface.

## Decision

**Use s-expressions with short ASCII keyword tags as the canonical surface form.**

Each node is `(kind child child ...)` where `kind` is a fixed-vocabulary keyword (`lam`, `app`, `let`, `rec`, `module`, `if`, `match`, `arm`, `record`, `proj`, `ctor`, `ann`, `var`, `int`, `str`, `sym`, `hole`, `pat-wild`, `pat-var`, `pat-ctor`) and children are nested s-expressions or atomic tokens. The full kind table lives in [canonical-text-format.md § 2](../plans/canonical-text-format.md#2-node-kinds).

The kind tag set is **frozen at Stage 2 exit**. Future evolution is additive only — new kinds get new tags; no tag is ever re-purposed.

## Alternatives considered

- **Integer kind IDs.** Smaller bytes per node tag (1–3 bytes vs. 3–8). Rejected because the size win is rounding error in a text format dominated by parens, spaces, and DeBruijn integers, while the readability cost during Phase 1 implementation and debugging is real and recurring. The argument that "AI doesn't need readability" is also weak — the canonical form is read by humans diagnosing canonicalizer bugs, not by LLMs at inference time (which read the authoring view).
- **JSON-style records.** Inflates byte size by 2–3× (every node carries `{"kind":` and `"children":[`). Rejected on size grounds and on parser-determinism grounds — JSON whitespace flexibility, escape variations, and key-order ambiguity all create cross-implementation divergence risk that has to be explicitly stamped out.
- **Custom binary.** Most compact, but writing two byte-equivalent implementations is significantly harder than for s-expressions, debug tooling has to be built from scratch, and any spec ambiguity manifests as opaque byte differences instead of visible text diffs. Rejected for Stage 2; revisitable in Phase 1+ if storage size becomes a bottleneck.
- **S-expressions with full-word kind tags (e.g. `lambda` instead of `lam`).** Costs ~2× per tag for no functional gain; rejected because the short tags are unambiguous and the kind table is small enough to memorize.

## Consequences

- **Two-implementation parsing is straightforward.** S-expression parsing is a few-hundred-line task in any language; byte-equivalence reduces to lexical-rule conformance ([ADR 0006](0006-canonical-lexical-rules.md)) and structural correctness.
- **Kind tag set is permanently additive.** Stage 2 exit freezes the table; renaming a tag breaks every stored content address. New kinds may be added in Phase 1+ via a follow-up ADR; existing tags are immutable.
- **Spec surface is small.** A single grammar production (s-expression of kind + children + atoms) covers the entire canonical form. No special cases for arity, no per-kind serialization rules.
- **Storage size is not optimized.** A future binary canonical form could be ~2–3× smaller. Acceptable for Phase 0; the Phase 1 storage layer can introduce a binary cache without changing the canonical hash domain (the hash is over canonical text bytes per [ADR 0009](0009-hashing-rule.md)).
- **The authoring view remains the BPE-optimized form.** Canonical text is *not* tokenizer-optimized; it is determinism-optimized. Most LLM interaction with Tacit code goes through the authoring view, not canonical.

## Related decisions

- [ADR 0003](0003-authoring-view-bpe-compact.md) — authoring view (different optimization target).
- [ADR 0004](0004-rec-arity.md) — `rec`/`module` arities, which this surface form encodes.
- [ADR 0006](0006-canonical-lexical-rules.md) — lexical rules (whitespace, integers, strings).
- [ADR 0009](0009-hashing-rule.md) — BLAKE3 over canonical text bytes.
