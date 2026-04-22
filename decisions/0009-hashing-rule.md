# 0009 — Hashing rule: BLAKE3 over inlined canonical text

**Status:** Accepted
**Date:** 2026-04-21
**Phase:** 0, Stage 2

## Context

The parent plan commits to BLAKE3 as the hash function for content-addressing. What remains to specify is *what bytes BLAKE3 sees*. Two designs:

- **(a) Inlined canonical text.** A node's hash is BLAKE3 over the full canonical text of that node, with all children's canonical text inlined.
- **(b) Hash references in canonical text.** A node's canonical text contains its children's *hashes* (in some agreed-upon byte form) rather than the children's full text. The hash is over this hash-referencing text.

Design (b) is more compact for very large ASTs (sibling subtree text doesn't bloat parent text) and matches the Merkle-DAG storage pattern used by Git, IPFS, etc. Design (a) is simpler to specify and debug.

## Decision

**Use design (a): `hash(node) = BLAKE3(canonical_text(node))`** with all children fully inlined in the parent's canonical text. No hash-reference syntax appears inside canonical text.

The hash output is the standard BLAKE3 32-byte digest of the UTF-8 byte sequence of canonical text. Representation of the hash (hex, base32, base64) is a sidecar/storage concern and is not part of this ADR.

## Alternatives considered

- **Hash references inline (design b).** Storage-efficient at scale but adds substantial spec surface: a syntax for hash references inside canonical text, a rule for when child text is inlined vs referenced (always referenced? above some size threshold?), an ordering between writing and hashing children, and a separate canonical byte-form for the hash itself. Two-implementation byte-equivalence becomes harder because any disagreement on hash byte-form cascades into parent text byte differences. Rejected for Stage 2; the size benefit doesn't materialize until programs are large, and at that point a Phase 1+ storage layer can dedupe by hash without touching the canonical-text spec. Revisitable if Phase 1 measurements show canonical text size becoming a bottleneck.
- **Hash over a binary serialization rather than text.** Would decouple the hash domain from text-format choices ([ADR 0006](0006-canonical-lexical-rules.md)). Rejected — adds a second canonical form (binary), doubling the spec surface and the implementation work, for no practical benefit while text is the canonical form.
- **Hash with domain separation per node kind.** BLAKE3 supports keyed hashing and key derivation. Could prevent collisions if canonical text from two unrelated contexts could ever match. Rejected as unnecessary — canonical text is unambiguous (the kind tag is part of every node's text), so a `(var 0)` node and an `(int 0)` node already produce different bytes.
- **Hash over the parsed AST structure rather than canonical text.** Sounds elegant ("hash the meaning, not the spelling") but pushes byte-equivalence onto the parser implementation rather than the text spec. Two implementations must agree on a canonical AST representation in memory — which is harder to verify than agreement on text bytes. Rejected; the canonical text *is* the canonical representation.

## Consequences

- **Hashing is a one-pass walk over canonical text bytes.** No separate "resolve hash references" or "compute child hashes first" step.
- **Subtrees are self-contained.** Any subtree's canonical text can be hashed independently, without needing access to the parent or siblings.
- **Sibling subtrees do not affect each other's hashes.** Editing one binding in a `rec` group does not change the hashes of the unedited bindings (their canonical text is unchanged), so cache invalidation is precise.
- **Parent hashes do change** when any descendant changes, because the parent's canonical text inlines the descendant's text. This is the Merkle-up-the-spine behavior we want.
- **Storage-layer dedup by hash works** without spec involvement. A Phase 1+ storage layer can keep one copy of an identical subtree's text (keyed by its hash) and reference it from multiple parents — but that's a storage-internal optimization invisible to the canonical-text spec.
- **Canonical text can be large** for large ASTs, since children are inlined. Phase 1 measurements will tell us whether this is a real problem; if it is, a binary canonical form or a hash-reference scheme can be introduced as an additional representation without retiring the current rule (the hash domain is text bytes, defined here).
- **`rec` groups hash as single atoms** as required by the parent plan: the entire `(rec ...)` text is one BLAKE3 input. The "hash as single atom" commitment is satisfied trivially by this rule.

## Related decisions

- [ADR 0005](0005-canonical-surface-form.md) — defines the s-expression text form that this rule hashes.
- [ADR 0006](0006-canonical-lexical-rules.md) — defines the byte sequence of canonical text exactly enough that BLAKE3 input is unambiguous.
- [ADR 0008](0008-record-field-ordering.md) — sorting rule that ensures semantically-equal records produce identical canonical text and therefore identical hashes.
