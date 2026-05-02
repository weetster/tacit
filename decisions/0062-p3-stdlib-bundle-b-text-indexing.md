# 0062 - Phase 3 stdlib Bundle B: text indexing

**Status:** Accepted
**Date:** 2026-05-02
**Phase:** 3, library-mediated experiment
**Requires:** [ADR 0061](0061-p3-stdlib-bundle-a-i64-vectors.md)
**Amends:** [ADR 0047](0047-p3-stdlib-expansion-surface.md) additively.

## Context

[ADR 0060](0060-p3-repair-loop-outcome.md) keeps the Phase 3 primer-only
gate failed and points library-mediated work at the remaining repair-loop
failure clusters. [ADR 0061](0061-p3-stdlib-bundle-a-i64-vectors.md) chooses
Bundle A as the first substrate: compact `i64` storage in `I64Vec`.

The proposed next-step plan in
[phase-3-stdlib-next-steps.md](../plans/phase-3-stdlib-next-steps.md)
identifies Bundle B as the text-indexing layer above that substrate. The
current text surface can scan bytes with `@scan-byte`, compare byte ranges
with `@buf-eq`, and store integers with `@i64-set`, but generated programs
still have to rediscover line and token boundaries by hand. That repeats
fragile offset arithmetic across line sorting, unique-line, word-count,
longest-line, and substring-filtering programs.

Bundle B should separate boundary discovery from per-task logic without
becoming a task-shaped library. It should not sort, deduplicate, count, or
filter. It should only produce compact range tables that later bundles and
ordinary Tacit code can consume.

## Decision

Add four new `@name` primitives:

| Category | `@name` | Arity | Type | Effect | Lowering |
|----------|---------|-------|------|--------|----------|
| `TEXT-INDEX` | `@line-index` | 3 | `Buf -> i64 -> I64Vec -> i64` | `{Mut}` | scan LF-delimited lines and write range pairs |
| `TEXT-INDEX` | `@token-index` | 5 | `Buf -> i64 -> i64 -> i64 -> I64Vec -> i64` | `{Mut}` | scan delimiter-separated non-empty byte runs |
| `RANGE-TABLE` | `@range-start` | 2 | `I64Vec -> i64 -> i64` | `{}` | load `table[2 * index]` |
| `RANGE-TABLE` | `@range-len` | 2 | `I64Vec -> i64 -> i64` | `{}` | load `table[2 * index + 1]` |

The signatures are exact. Underapplication and overapplication continue to
fail with primitive arity errors.

### Range-table layout

Bundle B uses the `I64Vec` type from ADR 0061. It does not introduce a new
`RangeTable` handle type.

A table row `i` occupies two `i64` elements:

- start: `table[2 * i]`
- length: `table[2 * i + 1]`

Starts are absolute byte offsets into the source `Buf`, not offsets relative
to the indexed subrange. Lengths are byte lengths. All row indexes are
zero-based.

The caller owns table allocation. The caller must allocate at least two
`i64` elements per produced range. If the table is too small, behavior is
undefined, matching the unchecked bounds discipline for `Buf` and `I64Vec`.
For worst-case canary programs, allocating `2 * len` `i64` elements is enough
for both `@line-index` and `@token-index`.

Rows at indexes greater than or equal to the returned count are uninitialized
and must not be read.

### Primitive semantics

`@line-index text len table` indexes the byte range `text[0..len)` into
LF-delimited lines. LF is byte value 10. The delimiter byte is not included in
the stored range.

Line indexing preserves empty lines between delimiters and at the start of
the input, but a final trailing LF does not create an extra empty line:

- empty input produces zero rows;
- `"\n"` produces one empty row at start 0 with length 0;
- `"a\n"` produces one row for `"a"`;
- `"a\n\n"` produces rows for `"a"` and the empty line between the two LF
  bytes; and
- a final unterminated segment is emitted when its length is greater than
  zero.

Only LF has delimiter meaning. CR bytes are ordinary bytes; Bundle B does not
normalize CRLF input.

`@token-index text off len delim table` indexes
`text[off..off+len)` into non-empty byte runs separated by the low byte of
`delim`. Leading, trailing, and repeated delimiters are skipped. Empty tokens
are not emitted. For example, delimiter byte 32 splits `" a  b "` into rows
for `"a"` and `"b"`.

`@range-start table index` returns the start field for row `index`.

`@range-len table index` returns the length field for row `index`.

For all four primitives, negative lengths, negative offsets, negative row
indexes, offset arithmetic overflow, and out-of-range access are undefined
behavior in Phase 3.

### Type system

`@line-index` and `@token-index` accept either fixed-size `Buf N` values or
dynamic `Buf` values for their text argument, following the Phase 3 memory
primitive convention from ADR 0047.

The table argument must be an `I64Vec`. The typechecker should reject all of
these:

- `@line-index text len byte_buf`
- `@token-index text off len 32 byte_buf`
- `@line-index i64_vec len table`
- `@range-start byte_buf 0`
- `@range-len byte_buf 0`

`I64Vec` keeps the scoped lifetime and anti-escape discipline from ADR 0061.
No new canonical node kind or type syntax is introduced.

### Effect integration

No new effect atom is introduced.

- `@line-index` and `@token-index` carry `{Mut}` because they mutate the
  caller-provided range table.
- `@range-start` and `@range-len` are pure (`{}`).

`stdlib/libc-effects.toml` is unchanged. These primitives do not cross an OS
boundary and do not add libc linkage.

### Codegen surface

`@line-index` and `@token-index` emit inline loops over `i8` buffer loads and
`i64` table stores. They do not call libc `strtok`, `memchr`, or `getline`.

`@range-start` and `@range-len` lower as direct `I64Vec` element loads. They
are primitive accessors, not macros, because the point of Bundle B is to stop
model-authored code from repeating pair-index arithmetic.

### Model-facing examples

The stdlib primer appendix must include at least these examples before a paid
canary:

```text
let text = @buf-alloc 128 in
let n = @read 0 text 128 in
let lines = @i64-alloc (@mul n 2) in
let line_count = @line-index text n lines in
if @eq line_count 0 then 0 else @range-len lines 0
```

```text
let text = @buf-alloc 128 in
let n = @read 0 text 128 in
let words = @i64-alloc (@mul n 2) in
let word_count = @token-index text 0 n 32 words in
word_count
```

Examples should describe table rows only as start/length pairs. They should
not mention corpus task names.

### Conformance tests

Implementation must add typecheck and codegen coverage for:

- primitive signatures and arities;
- rejection of `Buf`/`I64Vec` mixups;
- `@line-index` on empty input;
- `@line-index` with no trailing LF, trailing LF, consecutive LF bytes, and
  an empty first line;
- `@token-index` with empty input, leading delimiters, trailing delimiters,
  repeated delimiters, and a nonzero `off`;
- absolute start offsets for both indexers;
- accessor equivalence for `@range-start` and `@range-len`; and
- safe zero-row behavior when the input produces no ranges.

No canonical test vector is required. This bundle extends the `@name`
allowlist only.

## Alternatives considered

- **Introduce a distinct `RangeTable` type.** Rejected for Phase 3. ADR 0061
  already gives a typed integer-vector substrate, and a new table handle would
  add type-system and hidden-capture work before there is evidence that
  start/length pairs are insufficient.
- **Use two vectors, one for starts and one for lengths.** Rejected. Two
  handles increase allocation, capture, and argument pressure. A packed pair
  table keeps the representation compact and matches ADR 0061's model-facing
  convention.
- **Leave `@range-start` and `@range-len` as primer idioms.** Rejected. The
  repeated `2 * index` and `2 * index + 1` arithmetic is exactly the kind of
  small representation logic that generated programs get wrong and spend
  tokens on.
- **Make `@token-index` preserve empty fields.** Rejected. Bundle B's token
  primitive targets word-like token discovery. Empty-field preservation is a
  different CSV/table parsing operation and should be considered only if a
  later open-only experiment needs it.
- **Treat CRLF as a single line delimiter.** Rejected for the first bundle.
  Current text primitives are byte-oriented, the corpus convention is LF, and
  CRLF normalization would make stored byte lengths less direct.
- **Add sorting or counting over range tables now.** Rejected. Those are
  Bundle C and Bundle D concerns. Bundle B only creates reusable text ranges
  that can be consumed by later ordering and counting primitives.
- **Add explicit table capacity arguments.** Rejected for this experiment.
  Existing Phase 3 memory primitives use caller-managed bounds with undefined
  behavior on overflow. Capacity-aware truncation would add branches and return
  conventions that should be designed with a future safer collection library,
  not smuggled into the first text indexer.

## Consequences

- Bundle B depends on Bundle A's `I64Vec` implementation and should not be
  implemented first.
- Line and token discovery become one primitive call plus table accessors,
  reducing repeated scan-loop and pair-arithmetic code in generated Tacit.
- Range tables remain ordinary `I64Vec` values, so existing anti-escape and
  hidden-capture rules continue to apply.
- Bundle B does not make any stdlib-mediated result count as a primer-only
  Phase 3 pass. Results still need the `library-mediated` label or equivalent
  note from the stdlib plan.
- Existing core-language references remain comparable; Bundle B references
  must be added separately, for example as `reference.stdlib.tac`.

## Related

- [Phase 3 stdlib next steps](../plans/phase-3-stdlib-next-steps.md)
- [ADR 0047](0047-p3-stdlib-expansion-surface.md) - Phase 3 primitive
  expansion precedent
- [ADR 0060](0060-p3-repair-loop-outcome.md) - repair-loop outcome and next
  direction
- [ADR 0061](0061-p3-stdlib-bundle-a-i64-vectors.md) - Bundle A `I64Vec`
  substrate
