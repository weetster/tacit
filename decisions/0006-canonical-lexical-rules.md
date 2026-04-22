# 0006 — Canonical lexical rules: integers, whitespace, strings

**Status:** Accepted
**Date:** 2026-04-21
**Phase:** 0, Stage 2

## Context

Two implementations producing byte-identical canonical text requires every lexical choice — integer encoding, whitespace placement, string escaping — to have exactly one valid form. Any flexibility (optional whitespace, multiple integer representations, optional escapes) creates a cross-implementation divergence path.

The s-expression surface form ([ADR 0005](0005-canonical-surface-form.md)) sets the structural skeleton; this ADR pins the atomic-token and whitespace rules.

## Decision

**Integers.** Decimal ASCII. No leading zeros except the single digit `0`. Negative integers carry a leading `-`. No `+` sign, no thousands separators, no underscores, no alternative bases.

- Valid: `0`, `1`, `42`, `-7`, `2147483647`.
- Invalid: `01`, `+5`, `1_000`, `0x10`, `0b101`.

**Whitespace.**

- Exactly one ASCII space (`0x20`) between adjacent tokens at the same nesting level.
- Zero whitespace immediately after `(` or before `)`.
- Zero whitespace at the start or end of the entire canonical text. No trailing newline.
- No tabs, no CR, no other whitespace bytes anywhere in canonical text outside of string literals.

**Strings.** Double-quoted. Allowed escape sequences:

| Escape       | Meaning                          |
|--------------|----------------------------------|
| `\"`         | literal `"`                      |
| `\\`         | literal `\`                      |
| `\n`         | LF (`0x0A`)                      |
| `\t`         | TAB (`0x09`)                     |
| `\r`         | CR (`0x0D`)                      |
| `\u{HEX}`    | Unicode code point, 1–6 lower-case hex digits, no leading-zero requirement |

Raw newlines, raw tabs, and other raw control bytes are forbidden inside string literals — they must be escape-encoded. Source encoding is UTF-8. No Unicode normalization is performed; the byte sequence after escape decoding is what hashes.

**Symbols.** Bare ASCII identifiers in positions that take a name (record field, ctor name, hole diag-id). Match `[A-Za-z_][A-Za-z0-9_-]*`. No quoting, no escapes.

**Comments.** Forbidden. Comments live in the sidecar metadata, not in canonical text.

## Alternatives considered

- **LEB128 / varint integers.** Standard for binary formats; meaningless in a text format. Rejected with the surface-form choice ([ADR 0005](0005-canonical-surface-form.md)).
- **Pretty-printed multi-line canonical.** Improves human readability but kills determinism — line-wrap heuristics would have to be specified to byte precision, and any deviation cascades. Rejected; pretty-printing is the inspection view's job (Stage 3).
- **Optional whitespace.** "One or more spaces" instead of "exactly one space" simplifies hand-writing but destroys byte-equivalence. Rejected.
- **Allow trailing newline.** A common Unix convention, but the canonical text is rarely stored as a standalone file — it's a hash input. A trailing newline would be one more rule to get wrong. Rejected.
- **Permit raw newlines / tabs in strings.** Aligns with some source-code conventions (multi-line literals) but creates whitespace-handling ambiguity at parse time and complicates the lexer. Rejected; multi-line literals can be expressed via `\n` escapes, with no semantic loss.
- **Permit hex / binary integer literals.** Convenience for humans, but creates two valid forms for the same value (`16` vs `0x10`). The canonicalizer would have to normalize, adding spec surface. Rejected; users write whatever they want in the authoring view, canonicalizer normalizes to decimal.
- **Unicode normalization (NFC) before hashing.** Avoids the case where two visually-identical strings hash differently due to combining-mark variations. Rejected for Stage 2 because (a) it adds a Unicode-version dependency to the hash domain (NFC tables change between Unicode versions), (b) two implementations must agree on the same Unicode version to produce the same hash, and (c) the parser is a better place to enforce identifier well-formedness if needed. Canonical treats decoded string bytes as opaque.

## Consequences

- **Byte-comparison of canonical text is a sufficient cross-implementation conformance test.** No tolerance for whitespace, integer-format, or escape-form differences.
- **Hand-writing canonical text is annoying** (single space everywhere, no formatting). Acceptable: humans should rarely hand-write canonical; they write the authoring view. The canonicalizer produces canonical text.
- **The lexer is trivial.** A reader can tokenize by recognizing `(`, `)`, `"`-delimited strings, and runs of non-space-non-paren characters split by single spaces.
- **String content can carry any Unicode code point** via `\u{...}`, but the byte form is always ASCII-printable, simplifying transport.
- **Symbol vocabulary is restricted to ASCII**; non-ASCII identifiers in the authoring view get mapped to ASCII-safe symbols by the canonicalizer (mapping rule deferred — it's a Phase 1 problem, since Phase 0 grammars use ASCII identifiers).
- **No comment syntax in canonical** means the parser does not need to handle them; sidecar carries comments associated with subtree hashes.

## Related decisions

- [ADR 0005](0005-canonical-surface-form.md) — surface form (s-exprs with keyword tags).
- [ADR 0009](0009-hashing-rule.md) — BLAKE3 input is the canonical text bytes defined here.
