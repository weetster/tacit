# 0065 - Retroactive Phase 3 stdlib Bundle C: ordering primitives

**Status:** Accepted
**Date:** 2026-05-03
**Phase:** 3, library-mediated experiment
**Requires:** [ADR 0061](0061-p3-stdlib-bundle-a-i64-vectors.md),
[ADR 0062](0062-p3-stdlib-bundle-b-text-indexing.md)
**Amends:** [ADR 0047](0047-p3-stdlib-expansion-surface.md) additively.

## Context

This ADR records Bundle C after implementation. The Bundle C primitives landed
before a dedicated decision record was written, and
[ADR 0064](0064-p3-stdlib-bundle-d-search-counting.md) already assumes that
ordering over vectors and byte ranges exists. Numbering is therefore not
chronological: ADR 0065 is the retroactive record for the Bundle C surface that
sits between Bundle B/B2 and Bundle D.

[phase-3-stdlib-next-steps.md](../plans/phase-3-stdlib-next-steps.md)
identified ordering as the next pressure point after buffer-backed integer
vectors and range-table construction. Generated Tacit programs repeatedly
implemented fragile sorting loops for integer lists, line sorting, unique-line
preparation, and key/value reordering. Those loops consumed many tokens and
were a recurring source of compiler and behavioral repair failures.

Bundle C should provide only ordering. It should not discover text ranges,
deduplicate adjacent rows, count groups, perform binary search, or introduce
callbacks, closures, iterators, or a general collection library.

## Decision

Add three new `@name` primitives:

| Category | `@name` | Arity | Type | Effect | Lowering |
|----------|---------|-------|------|--------|----------|
| `ORDER` | `@sort-i64` | 2 | `I64Vec -> i64 -> i64` | `{Mut}` | stable in-place signed integer sort |
| `ORDER` | `@sort-ranges-by-bytes` | 3 | `Buf -> I64Vec -> i64 -> i64` | `{Mut}` | stable in-place byte-lexicographic range-row sort |
| `ORDER` | `@stable-sort-pairs-i64` | 3 | `I64Vec -> I64Vec -> i64 -> i64` | `{Mut}` | stable in-place key/value sort by signed integer key |

The signatures are exact. Underapplication and overapplication continue to
fail with primitive arity errors.

### Primitive semantics

`@sort-i64 xs count` sorts `xs[0..count)` in ascending signed `i64` order and
returns `0`. The vector is mutated in place. `count = 0` and `count = 1` leave
the vector unchanged.

`@sort-ranges-by-bytes text table count` sorts the first `count` range-table
rows by the bytes they reference in `text` and returns `0`. The range-table
layout is the ADR 0062 start/length pair layout:

- row `i` start: `table[2 * i]`
- row `i` length: `table[2 * i + 1]`

Rows are ordered by unsigned byte lexicographic comparison over
`text[start..start+length)`. If one range is a prefix of another, the shorter
range sorts first. Equal byte ranges keep their relative input order. The
primitive mutates only the row order in `table`; it does not mutate `text`.

`@stable-sort-pairs-i64 keys values count` sorts `keys[0..count)` in ascending
signed `i64` order, applies the same movement to `values[0..count)`, and
returns `0`. Equal keys keep their relative input order. The `keys` and
`values` vectors have distinct logical roles; aliasing them is undefined in
Phase 3.

For all three primitives, negative counts, negative row starts or lengths,
overflow, table or vector under-allocation, and out-of-range buffer/vector
access are undefined behavior in Phase 3. Rows or elements at indexes greater
than or equal to `count` are outside the sorted prefix and must be left
unchanged.

### Type system

`@sort-i64` accepts only an `I64Vec` as its first argument.

`@sort-ranges-by-bytes` accepts a byte `Buf` for `text` and an `I64Vec` for
`table`. The `Buf` argument follows the Phase 3 buffer convention from ADR
0047: fixed-size and dynamic byte buffers are both usable where `Buf` is
expected.

`@stable-sort-pairs-i64` accepts only `I64Vec` handles for both the keys and
values arguments.

The typechecker should reject all of these:

- `@sort-i64 byte_buf count`
- `@sort-ranges-by-bytes i64_vec table count`
- `@sort-ranges-by-bytes text byte_buf count`
- `@stable-sort-pairs-i64 keys byte_buf count`

No new built-in type, canonical node kind, type syntax, effect atom, or libc
signature is introduced.

### Effect integration

All three primitives carry `{Mut}` because they mutate caller-provided storage.
They do not allocate and do not perform I/O.

`stdlib/libc-effects.toml` is unchanged. These primitives do not cross an OS
boundary and do not add libc linkage.

### Codegen

The implemented lowering emits inline stable insertion-sort loops. It does not
call libc `qsort`, does not require function pointers, and does not introduce a
comparator callback ABI.

The stable insertion-sort lowering is sufficient for the Phase 3 open corpus
and canary sizes. A future implementation may replace the lowering with a more
efficient stable algorithm as long as the observable semantics above are
preserved.

### Model-facing examples

The stdlib primer appendix must describe these shapes without task-specific
names:

```text
let xs = @i64-alloc 3 in
let _ = @i64-set xs 0 9 in
let _ = @i64-set xs 1 -2 in
let _ = @i64-set xs 2 4 in
let _ = @sort-i64 xs 3 in
@i64-get xs 0
```

```text
let text = @buf-alloc 128 in
let n = @read 0 text 128 in
let rows = @i64-alloc (@mul n 2) in
let row_count = @line-index text n rows in
let _ = @sort-ranges-by-bytes text rows row_count in
row_count
```

```text
let keys = @i64-alloc 4 in
let values = @i64-alloc 4 in
let _ = @stable-sort-pairs-i64 keys values 4 in
@i64-get values 0
```

Examples should state that range sorting reorders start/length rows while
leaving source bytes in place, and that pair sorting preserves equal-key order.

### Conformance tests

The implementation landed with typecheck and codegen coverage for:

- primitive signatures, arities, and `{Mut}` effects;
- rejection of `Buf`/`I64Vec` mixups;
- `@sort-i64` on signed values, duplicates, and zero rows;
- `@sort-ranges-by-bytes` on ordinary lexicographic order and prefix ordering;
- `@stable-sort-pairs-i64` preserving attached values and equal-key order; and
- primer fixture coverage for all three model-facing signatures.

Follow-up changes to Bundle C should add explicit equal-range, one-row, and
outside-prefix mutation fixtures if they change the lowering.

No canonical test vector is required. This bundle extends the `@name` allowlist
only.

## Alternatives considered

- **Keep sorting in Tacit reference programs.** Rejected. Hand-written sorting
  loops were long, fragile, and central to the remaining repair-loop failure
  clusters that motivated the library-mediated experiment.
- **Add only `@sort-i64`.** Rejected. Integer sorting helps numeric tasks but
  leaves line sorting, unique-line preparation, and token-range grouping
  reimplementing byte-range comparisons by hand.
- **Use libc `qsort`.** Rejected. It would add a libc dependency and require a
  comparator callback surface that Tacit-Lite does not have. Inline lowering
  keeps ADR 0025's OS-boundary discipline intact.
- **Introduce a generic comparator or closure-based sort.** Rejected. Tacit
  does not yet have the closure, polymorphism, or callback ABI needed to make a
  general sort primitive honest.
- **Make the range sort parse or normalize text.** Rejected. Bundle C is an
  ordering layer over byte ranges. Line/token discovery belongs to Bundle B/B2,
  and grouping/deduplication belongs to Bundle D.
- **Use unstable sorts.** Rejected. Stable pair sorting is required to keep
  attached values in input order for equal keys, and stable range sorting keeps
  later adjacent grouping deterministic.

## Consequences

- Bundle C depends on Bundle A's `I64Vec` substrate and Bundle B's range-table
  representation.
- Sorting-heavy stdlib references can use one primitive call instead of
  model-authored nested loops.
- Bundle D can assume sorted integer vectors or sorted byte-range tables before
  applying binary search, adjacent deduplication, or adjacent counting.
- Library-mediated results still need separate reporting and do not satisfy
  primer-only Phase 3 gates.

## Related

- [Phase 3 stdlib next steps](../plans/phase-3-stdlib-next-steps.md)
- [ADR 0047](0047-p3-stdlib-expansion-surface.md) - Phase 3 primitive
  expansion precedent
- [ADR 0061](0061-p3-stdlib-bundle-a-i64-vectors.md) - Bundle A `I64Vec`
  substrate
- [ADR 0062](0062-p3-stdlib-bundle-b-text-indexing.md) - Bundle B range tables
- [ADR 0064](0064-p3-stdlib-bundle-d-search-counting.md) - Bundle D consumers
  of sorted vectors and range tables
