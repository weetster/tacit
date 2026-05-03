# 0063 - Phase 3 stdlib Bundle B2: token-index-any

**Status:** Accepted
**Date:** 2026-05-03
**Phase:** 3, library-mediated experiment
**Requires:** [ADR 0061](0061-p3-stdlib-bundle-a-i64-vectors.md)
**Amends:** [ADR 0062](0062-p3-stdlib-bundle-b-text-indexing.md) additively.

## Context

ADR 0062 added `@token-index text off len delim table`, which splits on one
delimiter byte. The first library-mediated canary showed that this is too
narrow for ordinary tokenized input: spaces, LF, CR, and tabs may all separate
tokens in the same input range. Generated programs then either used the one
delimiter primitive incorrectly or returned to fragile byte-scanning loops.

Bundle B2 keeps Bundle B's range-table model but adds a multi-delimiter token
indexer. It remains a boundary-discovery primitive only; sorting, counting,
deduplication, and parsing stay outside this decision.

## Decision

Add one `TEXT-INDEX` primitive:

| Category | `@name` | Arity | Type | Effect | Lowering |
|----------|---------|-------|------|--------|----------|
| `TEXT-INDEX` | `@token-index-any` | 6 | `Buf -> i64 -> i64 -> Buf -> i64 -> I64Vec -> i64` | `{Mut}` | scan non-empty byte runs separated by any delimiter byte |

`@token-index-any text off len delims delim-count table` scans
`text[off..off+len)` into non-empty byte runs separated by any byte in
`delims[0..delim-count)`. Leading, trailing, and repeated delimiter bytes are
skipped. Stored starts are absolute byte offsets into `text`, matching ADR
0062. Lengths are byte lengths. The return value is the number of rows written.

If `delim-count` is zero, no byte is a delimiter and a non-empty range emits
one token. Negative offsets, negative lengths, negative delimiter counts,
overflow, and out-of-range delimiter or table access are undefined behavior in
Phase 3.

The `text` argument must be a byte buffer handle. The `delims` argument may be
either a byte buffer handle or a string literal. For a string literal, bytes are
used exactly as written after ordinary string escape decoding; the caller still
passes `delim-count` explicitly. `delim-count` may be smaller than the literal
length to use a prefix. Passing a larger count is out-of-range access and has
undefined behavior.

The one-delimiter `@token-index` remains available as a compact low-level
primitive. Model-facing examples should use `@token-index-any` when more than
one separator byte may appear.

## Type System

Full applications of `@token-index-any` must reject:

- non-buffer `text` arguments;
- non-buffer and non-string-literal `delims` arguments;
- non-`I64Vec` tables; and
- arity other than six.

The primitive carries `{Mut}` because it mutates the caller-provided range
table. It does not introduce a new effect atom and does not change
`stdlib/libc-effects.toml`.

## Codegen

`@token-index-any` lowers to inline loops over `i8` buffer loads and `i64`
range-table stores. The delimiter membership check scans
`delims[0..delim-count)` directly. It does not call libc tokenization or search
functions.

## Conformance Tests

Implementation must add typecheck and codegen coverage for:

- primitive arity and signature;
- delimiter arguments as both a byte buffer and a string literal;
- rejection of byte-buffer/I64Vec mixups;
- empty input;
- leading, trailing, repeated, and mixed delimiters including space plus LF;
- nonzero `off` with absolute stored starts; and
- `delim-count = 0`.

## Alternatives Considered

- **Replace `@token-index`.** Rejected. The one-byte variant remains compact
  and useful when input is known to have one delimiter byte.
- **Hard-code whitespace.** Rejected. A delimiter set keeps the primitive
  byte-oriented and useful for comma/semicolon and other simple token streams.
- **Require delimiters only in buffers.** Rejected. Small delimiter sets such
  as `" \n\r\t"` are clearer and shorter as literals.

## Consequences

- Text tokenization examples can cover ordinary mixed-separator input without
  hand-written delimiter loops.
- The range-table representation, allocation discipline, and undefined bounds
  behavior from ADR 0062 remain unchanged.
- Library-mediated results still need separate reporting and do not satisfy
  primer-only Phase 3 gates.

## Related

- [Phase 3 stdlib next steps](../plans/phase-3-stdlib-next-steps.md)
- [ADR 0062](0062-p3-stdlib-bundle-b-text-indexing.md) - Bundle B text indexing
- [ADR 0061](0061-p3-stdlib-bundle-a-i64-vectors.md) - Bundle A `I64Vec`
  substrate
