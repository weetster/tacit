# 0064 - Phase 3 stdlib Bundle D: search and counting helpers

**Status:** Accepted
**Date:** 2026-05-03
**Phase:** 3, library-mediated experiment
**Requires:** [ADR 0061](0061-p3-stdlib-bundle-a-i64-vectors.md),
[ADR 0062](0062-p3-stdlib-bundle-b-text-indexing.md)
**Amends:** [ADR 0047](0047-p3-stdlib-expansion-surface.md) additively.

## Context

Bundle A added compact integer storage, Bundle B/B2 added range-table
construction, and Bundle C added ordering over vectors and byte ranges. That
still leaves generated programs repeating fragile grouping, deduplication, and
binary-search loops after sorting.

Bundle D provides the smallest reusable accumulation layer without adding
closures, iterators, or hash maps. It assumes the caller has already indexed or
sorted the relevant data, then performs common searches or adjacent grouping
over existing `I64Vec` storage.

## Decision

Add three new `@name` primitives:

| Category | `@name` | Arity | Type | Effect | Lowering |
|----------|---------|-------|------|--------|----------|
| `SEARCH` | `@lower-bound-i64` | 3 | `I64Vec -> i64 -> i64 -> i64` | `{}` | binary search over a sorted vector prefix |
| `RANGE-GROUP` | `@count-equal-ranges` | 4 | `Buf -> I64Vec -> i64 -> I64Vec -> i64` | `{Mut}` | adjacent equal range grouping into triples |
| `RANGE-GROUP` | `@dedup-adjacent-ranges` | 4 | `Buf -> I64Vec -> i64 -> I64Vec -> i64` | `{Mut}` | adjacent equal range compaction into pairs |

The signatures are exact. Underapplication and overapplication continue to
fail with primitive arity errors.

### Primitive semantics

`@lower-bound-i64 xs count value` returns the smallest index `i` in
`[0, count]` such that every sorted element before `i` is less than `value`
and every sorted element from `i` onward is greater than or equal to `value`.
The comparison is signed `i64`. If all inspected elements are less than
`value`, it returns `count`. The caller must provide `xs[0..count)` sorted in
ascending order.

`@dedup-adjacent-ranges text table count out` scans the first `count` rows of
`table` and writes one start/length pair to `out` for each run of adjacent
equal byte ranges. Equality requires equal lengths and byte-for-byte equality
inside `text`. The first row in each run is preserved. The return value is the
number of rows written. `out` may alias `table`, enabling in-place adjacent
deduplication.

`@count-equal-ranges text table count out` scans the first `count` rows of
`table` and writes one three-column row to `out` for each run of adjacent equal
byte ranges:

- `out[3 * row]`: representative start;
- `out[3 * row + 1]`: representative length; and
- `out[3 * row + 2]`: run count.

The return value is the number of grouped rows written. The caller must provide
an `out` vector distinct from `table`; aliasing is undefined for this
primitive because the output row width differs from the input row width.

For all three primitives, negative counts, negative row indexes, overflow, and
out-of-range access are undefined behavior in Phase 3.

### Type system

`@lower-bound-i64` accepts only an `I64Vec` as its first argument. It is pure.

`@count-equal-ranges` and `@dedup-adjacent-ranges` accept only a byte `Buf` for
`text` and `I64Vec` handles for both range tables. They carry `{Mut}` because
they mutate the caller-provided output vector.

No new built-in type, canonical node kind, effect atom, or libc signature is
introduced.

### Codegen

`@lower-bound-i64` lowers to an inline half-open binary-search loop over
`xs[0..count)`.

`@count-equal-ranges` and `@dedup-adjacent-ranges` lower to inline loops over
range rows and byte comparisons. They do not call libc string, sorting, or
hashing functions.

### Model-facing examples

The stdlib primer appendix must describe these shapes without task-specific
names:

```text
let xs = @i64-alloc 4 in
let _ = @i64-set xs 0 1 in
let _ = @i64-set xs 1 3 in
let _ = @i64-set xs 2 3 in
let _ = @i64-set xs 3 9 in
@lower-bound-i64 xs 4 3
```

```text
let text = @buf-alloc 5 in
let rows = @i64-alloc 6 in
let unique = @i64-alloc 6 in
let count = @dedup-adjacent-ranges text rows 3 unique in
count
```

For counted range groups, examples should state that output rows are triples,
not start/length pairs.

### Conformance tests

Implementation must add typecheck and codegen coverage for:

- primitive signatures and arities;
- rejection of `Buf`/`I64Vec` mixups;
- lower-bound hits, insertion points, and empty prefixes;
- adjacent range deduplication with repeated and distinct rows;
- zero-row range grouping; and
- counted range output triples.

No canonical test vector is required. This bundle extends the `@name`
allowlist only.

## Alternatives considered

- **Add a hash-map primitive.** Rejected. Hash maps would require a larger
  collection design, allocation model, and collision semantics. Adjacent
  grouping composes with the existing sort primitives.
- **Make `@count-equal-ranges` preserve only counts.** Rejected. Grouped text
  output needs both a representative byte range and its count.
- **Use range-pair output for counted groups.** Rejected. Counts need a third
  column; overloading the length field would make output ambiguous.
- **Require sorted input inside the counting primitives.** Rejected. They only
  depend on adjacent equality. Sorting remains Bundle C's responsibility.

## Consequences

- Tacit can express common sorted grouping and uniqueness flows with one
  primitive call after range indexing and sorting.
- Counted range output uses a documented triple layout while ordinary range
  tables keep the existing start/length pair layout.
- Library-mediated results remain separate from primer-only Phase 3 gates.

## Related

- [Phase 3 stdlib next steps](../plans/phase-3-stdlib-next-steps.md)
- [ADR 0061](0061-p3-stdlib-bundle-a-i64-vectors.md) - Bundle A `I64Vec`
  substrate
- [ADR 0062](0062-p3-stdlib-bundle-b-text-indexing.md) - Bundle B range tables
- [ADR 0063](0063-p3-stdlib-bundle-b2-token-index-any.md) - multi-delimiter
  token indexing
