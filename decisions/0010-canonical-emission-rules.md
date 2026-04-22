# 0010 — Canonical emission rules for atoms (strings and integers)

**Status:** Accepted
**Date:** 2026-04-21
**Phase:** 0, Stage 2

## Context

[ADR 0006](0006-canonical-lexical-rules.md) pins parser-level validity for integers, strings, and symbols in canonical text: what the parser accepts. Byte-exact canonicalization requires more — for every AST node, exactly one canonical byte sequence must emit. Parser permissiveness and canonicalizer emission are separate concerns, and Stage 2's exit criterion (two implementations produce byte-identical canonical text) binds on the latter.

Stage 2 test-vector drafting (`plans/test-vectors-draft.md`, vectors 9, 13, 14, 15) surfaced five specific gaps in ADR 0006's emission story:

1. **String `\u{HEX}` emission form.** ADR 0006 allows 1–6 hex digits with no leading-zero requirement. `\u{a}`, `\u{0a}`, `\u{00000a}` all parse to U+000A. Emission is not pinned.
2. **Raw vs escaped non-ASCII in strings.** ADR 0006 forbids only raw newlines, tabs, and controls — implicitly permitting raw UTF-8 non-ASCII. A string containing `😀` can legally emit as raw 4-byte UTF-8 or as `\u{1f600}`.
3. **Named-escape vs `\u{HEX}` for ASCII specials.** `"` (0x22) can emit as `\"` or `\u{22}`; same for `\`, LF, TAB, CR.
4. **Signed zero `-0`.** Syntactically valid under ADR 0006's integer grammar, semantically identical to `0`.
5. **Integer range.** ADR 0006 places no bound on integer magnitude. Two canonicalizers using different internal int types (u64 vs i128 vs bignum) can diverge on very large values.

Strings and integers are the two atomic-value kinds in canonical text. This ADR layers emission rules on both, in one decision, because they share the same class of problem (one semantic value, multiple legal parse forms).

## Decision

### Strings

**S1. Named-escape preference.** If the byte being emitted has a named escape, the canonicalizer emits the named escape:

| Byte       | Emission |
|------------|----------|
| 0x22 `"`   | `\"`     |
| 0x5c `\`   | `\\`     |
| 0x09 TAB   | `\t`     |
| 0x0a LF    | `\n`     |
| 0x0d CR    | `\r`     |

**S2. Escape all non-ASCII and unnamed controls.** Any byte outside ASCII-printable (0x20–0x7e) that does not match S1 is emitted as `\u{HEX}`. This covers 0x00–0x08, 0x0b, 0x0c, 0x0e–0x1f, 0x7f, and all code points ≥ 0x80.

**S3. `\u{HEX}` form.** Lowercase hex, minimum digits, no leading zeros (the single literal `0` for U+0000 is the one exception — `\u{0}` is the canonical form for NUL). Examples: `\u{a0}`, `\u{1f600}`, `\u{10ffff}`.

**S4. Direct emission otherwise.** Bytes in 0x20–0x7e that are not `"` (0x22) or `\` (0x5c) emit as themselves.

**Consequence:** Canonical string bytes between the surrounding `"…"` are always 7-bit-ASCII-printable, honoring ADR 0006's "simplifies transport" consequence.

### Integers

**I1. `-0` normalization.** The canonicalizer treats any AST-level "negative zero" as identical to `0` and emits `0`. Canonical text never contains the two-byte sequence `-0` as a complete integer token. Standalone `0` remains; `-N` for any N ≥ 1 remains.

**I2. Arbitrary precision.** Canonical text accepts integer literals of any magnitude matching ADR 0006's decimal grammar. Canonicalizer implementations must use arbitrary-precision integers at the canonical layer — no bounded integer type may truncate, wrap, or panic during canonicalization. Runtime integer representation (what the Tacit-Lite evaluator and any Phase 1+ backend use) is a separate concern and may be bounded.

## Alternatives considered

- **Pad `\u{HEX}` to fixed width (4 or 6 digits).** Easier to eyeball in diffs, and aligns with some language specs. Rejected: wastes bytes and conflicts with S1, since common ASCII controls would be padded-hex instead of named escapes.
- **Preserve user's source form for strings** (pass through whatever the parser saw). Rejected: two users writing the same semantic string differently would hash differently, directly violating the content-addressing intent. ADR 0008 already commits to normalizing away one dimension of user choice (record field order); strings are the same class.
- **Raw UTF-8 for non-ASCII.** Smaller bytes (1–4 per code point vs 5–10 for `\u{}`). Rejected: ADR 0006's "canonical text is always ASCII-printable" consequence is worth the byte cost, and strict ASCII resists transport/editor mangling (BOMs, unhelpful NFC normalization, non-UTF-8-aware tooling).
- **Accept `-0` as distinct from `0`.** Defensible if Tacit-Lite had IEEE-754 signed-zero integer semantics. It does not. Rejected — creates two canonical forms for one value.
- **Bound integer range to i64 / i128 at the canonical layer.** Matches what the Phase 1 runtime will likely use. Rejected: canonical text is the hash domain, not the runtime representation. Pinning a range here forces Phase 1 either to match or to add a second validation pass. Arbitrary precision at canonical + bounded at runtime cleanly separates the two.
- **One ADR per gap (five ADRs).** Rejected on the combine-related-items principle: all five are "one semantic value, multiple legal parse forms, pick one emission rule." A single ADR keeps the rationale in one place.

## Consequences

- **Two independent canonicalizers can agree byte-exactly on all string and integer atoms.** The five gaps collapse to one emission function each, deterministic in both directions.
- **ADR 0006 is not amended or superseded.** This ADR layers emission rules on top of its parser rules.
- **Canonical text is strictly 7-bit ASCII.** Useful for transport, grepping, diffing. Confirms ADR 0006's earlier claim.
- **Bignum dependency for canonicalizer implementations.** Rust: `num-bigint` or similar. Python: native `int`. Small cost in both ecosystems.
- **Test vectors 9, 13, 14, 15 in `plans/test-vectors-draft.md` pin to single canonical forms.** Stage 2 exit criterion 1 (precision sufficient for byte-identical two-impl output) is satisfied for atoms.
- **Escape-form choice is invisible to Tacit-Lite semantics.** The runtime sees decoded bytes; canonical representation is purely identity/transport.

## Related decisions

- [ADR 0006](0006-canonical-lexical-rules.md) — parser-level validity for atoms. This ADR layers emission-level pinning on top, without modifying ADR 0006.
- [ADR 0008](0008-record-field-ordering.md) — parallel case where the canonicalizer overrides user choice (field order) to preserve hash-equality of semantic-equality. Strings follow the same principle.
- [ADR 0009](0009-hashing-rule.md) — pinning atom emission ensures the BLAKE3 input is fully determined by the AST, not by any upstream source form.
- [ADR 0011](0011-variable-arity-minimums.md) — parallel structural-minimum pinning, landing at the same time.
