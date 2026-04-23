# Decision Log

Architecture Decision Records (ADRs) for Tacit. One file per decision, numbered sequentially.

## Format

Each ADR uses the structure:

- **Status** — Proposed / Accepted / Superseded by NNNN / Deprecated
- **Context** — the problem and constraints
- **Decision** — what we chose
- **Alternatives considered** — what we rejected, and why
- **Consequences** — what this commits us to, and what becomes easier or harder

Keep ADRs short. They record the decision, not the full analysis that led to it.

## Index

- [0001 — Target tokenizer](0001-target-tokenizer.md)
- [0002 — License](0002-license.md)
- [0003 — Authoring view format: BPE-compact](0003-authoring-view-bpe-compact.md)
- [0004 — `rec` arity: 1+N with a separate `module` kind](0004-rec-arity.md)
- [0005 — Canonical surface form: s-expressions with keyword tags](0005-canonical-surface-form.md)
- [0006 — Canonical lexical rules: integers, whitespace, strings](0006-canonical-lexical-rules.md)
- [0007 — DeBruijn indexing convention for `rec` and `module`](0007-debruijn-rec-indexing.md)
- [0008 — Record field ordering: sorted by field-symbol bytes](0008-record-field-ordering.md)
- [0009 — Hashing rule: BLAKE3 over inlined canonical text](0009-hashing-rule.md)
- [0010 — Canonical emission rules for atoms (strings and integers)](0010-canonical-emission-rules.md)
- [0011 — Minimum arity for variable-arity kinds](0011-variable-arity-minimums.md)
- [0012 — String code-point restriction: Unicode scalar values only](0012-unicode-scalar-value-restriction.md)
- [0013 — Canonical text format frozen](0013-canonical-text-format-frozen.md)
- [0014 — Display metadata sidecar: JSON parallel tree](0014-sidecar-format.md)
- [0015 — Inspection view scope: display-only pseudo-code with progressive annotations](0015-inspection-view-scope.md)
- [0016 — Rust AST enum: spec-conformant, in-place in the canonicalizer](0016-rust-ast-enum-location.md)
- [0017 — Stage 3 view-system spec frozen](0017-stage-3-frozen.md)
- [0018 — Stage 5 repository scaffolding frozen](0018-stage-5-frozen.md)
- [0019 — Corpus reference-solution idiom rules for Python and Rust](0019-corpus-idiom-rules.md)
- [0020 — Seal held-out corpus tasks in-repo via multi-layer guardrails](0020-sealing-held-out-in-repo.md)
