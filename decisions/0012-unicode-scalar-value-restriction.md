# 0012 — String code-point restriction: Unicode scalar values only

**Status:** Accepted
**Date:** 2026-04-22
**Phase:** 0, Stage 2
**Tightens:** [ADR 0006 — Canonical lexical rules](0006-canonical-lexical-rules.md) (string-escape clause only; rest of 0006 unaffected)

## Context

[ADR 0006](0006-canonical-lexical-rules.md)'s string-escape table accepts `\u{HEX}` for "1–6 lower-case hex digits, no leading-zero requirement." Read literally, that permits two classes of code point that are not valid Unicode scalar values:

1. **Surrogate code points** U+D800–U+DFFF — reserved for UTF-16 encoding of supplementary-plane characters; not real characters themselves. No valid UTF-8 byte sequence encodes them.
2. **Out-of-range values** above U+10FFFF — up to `\u{ffffff}` is syntactically accepted by the 6-hex-digit limit, but Unicode is defined only through U+10FFFF. `\u{110000}`–`\u{ffffff}` are ~15M code points with no meaning.

Stage 2 test-vector drafting (`plans/test-vectors-draft.md`, Vector 24) surfaced this as a two-implementation divergence risk. A canonicalizer built on `Rust String` or `Python str` enforces scalar-value validity at the language level — constructing `"\u{d800}"` fails with `InvalidData` / `UnicodeDecodeError`. A canonicalizer built on raw byte arrays has no such backstop and silently accepts these forms. Two implementations fed the same input produce different behavior: Stage 2's exit criterion ("byte-identical output") fails not because of disagreement on canonical bytes, but because one implementation refuses inputs the other accepts.

The choice needed was pre-empting this by defining which code-point values are syntactically valid at all.

## Decision

**A `\u{HEX}` escape's hex value must be a Unicode scalar value: in U+0000–U+D7FF or U+E000–U+10FFFF.** All other 1–6-hex-digit values are syntactically invalid canonical text. Specifically forbidden:

- **Surrogates:** U+D800–U+DFFF inclusive (2048 code points).
- **Out-of-range:** any hex value > U+10FFFF.

**Rejection is a hard parse error, not a `(hole ...)` node.** A parser encountering `\u{d800}` or `\u{110000}` fails the containing string literal and propagates the error upward — it does not construct an AST containing an invalid-string-literal hole.

The § 3 Strings subsection of `plans/canonical-text-format.md` restates this rule inline so the spec reads correctly without requiring the reader to chase the ADR.

## Alternatives considered

- **Leave the spec as-is; let each implementation diverge.** Rejected — Stage 2's entire purpose is byte-equivalence. Known divergence paths are spec bugs.
- **Permit surrogates; require canonical text encode them via some escape form.** Technically possible (e.g., via explicit UTF-16 surrogate pairs), but pointless: no valid Unicode string contains a surrogate, no authoring-view program would generate one, and the canonical form would grow a special case that serves no program. Rejected.
- **Produce `(hole invalid-escape (str "..."))` instead of hard-erroring.** Consistent with § 7's hole philosophy for *parser*-level failures (unexpected token, unclosed paren, etc.). Rejected for two reasons: (a) the failure is strictly lexical — the escape is malformed within a token, not a higher-level structural problem — and lexer-level failures in Tacit-Lite already hard-error (malformed integers, unterminated strings, raw control bytes in string literals); (b) adding `invalid-escape` to the § 7 diag-id table expands the surface ADR 0006 explicitly froze. Hard error keeps the diag-id set stable and aligns the treatment of malformed `\u{...}` with its siblings.
- **Amend ADR 0006 inline instead of superseding.** Rejected per the decision-log convention (decisions/README.md): ADRs are frozen once accepted. This ADR tightens a single clause of 0006 and says so in the header; 0006's status stays "Accepted," not "Superseded," because only one clause changes. A follow-up ADR that rewrote the entire lexical-rules document *would* supersede 0006.
- **Defer to Phase 1's parser.** Rejected — Stage 2 must produce a spec precise enough that two implementations produce identical output. Leaving this open pushes the divergence path into Phase 1, violating the spec-first commitment.

## Consequences

- **§ 3 Strings in `canonical-text-format.md` adds one paragraph** restating the scalar-value restriction; ADR 0006's escape table is otherwise unchanged.
- **Vector 24 becomes a confirmed anti-test.** `(str "\u{d800}")` and `(str "\u{110000}")` must be rejected at parse time by any conformant implementation. The vector's "candidate canonical" framing is replaced with explicit forbidden-forms framing, in the style of Vector 10c/d/e.
- **Lexer rules gain one validation step.** After accumulating the hex digits of a `\u{...}` escape, the lexer must check the parsed value against the scalar-value range before emitting the decoded code point. One comparison; cheap.
- **Diag-id set from § 7 is not expanded.** `invalid-escape` is not added; malformed escapes are lexer-level hard errors.
- **Existing canonical text is unaffected.** No canonical text emitted before this ADR could have contained a surrogate or out-of-range escape without being already malformed on every scalar-enforcing platform; tightening the spec breaks no valid content.
- **ADR 0006's escape table description is slightly out of date** (says "1–6 lower-case hex digits, no leading-zero requirement" with no scalar-value restriction). The spec document is the canonical reference; ADR 0006's table remains historically accurate to what was decided on 2026-04-21 and should not be retroactively edited. Readers consulting ADR 0006 directly will need to follow the tightening reference in this ADR's header.

## Related decisions

- [ADR 0006](0006-canonical-lexical-rules.md) — tightened. Parser-accept clause for `\u{HEX}` gets the Unicode-scalar-value restriction layered on top.
- [ADR 0010](0010-canonical-emission-rules.md) — emission rules for `\u{HEX}`; unaffected, since the canonicalizer never emits surrogates or out-of-range values (it canonicalizes valid Unicode source bytes, which are scalar values by construction).
- § 7 of `plans/canonical-text-format.md` — hole diag-id table. Explicitly *not* expanded by this ADR.
