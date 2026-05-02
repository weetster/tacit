# 0061 - Phase 3 stdlib Bundle A: buffer-backed i64 vectors

**Status:** Accepted
**Date:** 2026-05-02
**Phase:** 3, library-mediated experiment
**Amends:** [ADR 0038](0038-p2-writable-buffer.md),
[ADR 0047](0047-p3-stdlib-expansion-surface.md), and
[ADR 0059](0059-p3-rec-hidden-captures.md) additively.

## Context

[ADR 0060](0060-p3-repair-loop-outcome.md) keeps the Phase 3 primer-only
gate failed and points standard-library expansion at the remaining open
repair-loop failure clusters. The proposed next experiment in
[phase-3-stdlib-next-steps.md](../plans/phase-3-stdlib-next-steps.md)
starts with Bundle A: compact integer-sequence storage.

The existing Phase 3 memory surface is byte-oriented:

- `@buf-get buf off` loads one byte and zero-extends it to `i64`.
- `@buf-set buf off byte` stores the low byte of an `i64`.
- `@buf-copy` copies byte ranges.

Those primitives are the right substrate for text and raw I/O, but they are
the wrong authoring surface for integer sequences. Sorting, grouping, matrix,
partitioning, and range-table programs currently either rescan input to
recover values or hand-pack integers into byte buffers. That loses signed
values, fails for values above 255, and burns many tokens on representation
code instead of task logic.

Bundle A operates on `i64`, not `i8`, because Tacit-Lite's scalar `Int`
already lowers as `i64`, `@parse-i64` returns `i64`, and `@fmt-i64`
consumes `i64`. An `i8` vector bundle would duplicate the existing byte
buffer surface rather than solve the integer-sequence problem.

The bundle also needs an allocator. Requiring authors to allocate
`8 * count` bytes with `@buf-alloc-dyn` preserves the same representation
mistakes this bundle is meant to remove.

## Decision

Add a new opaque stack-buffer handle type, `I64Vec`, plus five new `@name`
primitives:

| Category | `@name` | Arity | Type | Effect | LLVM lowering |
|----------|---------|-------|------|--------|---------------|
| `I64VEC-ALLOC` | `@i64-alloc` | 1 | `i64 -> I64Vec` | `{Alloc}` | `alloca i64, count`; let-RHS only |
| `I64VEC` | `@i64-get` | 2 | `I64Vec -> i64 -> i64` | `{}` | `gep i64` + `load i64` |
| `I64VEC` | `@i64-set` | 3 | `I64Vec -> i64 -> i64 -> i64` | `{Mut}` | `gep i64` + `store i64`; returns 0 |
| `I64VEC` | `@i64-swap` | 3 | `I64Vec -> i64 -> i64 -> i64` | `{Mut}` | two loads + two stores; returns 0 |
| `I64VEC` | `@i64-copy` | 5 | `I64Vec -> i64 -> I64Vec -> i64 -> i64 -> i64` | `{Mut}` | element copy; returns 0 |

The signatures are exact. Underapplication and overapplication continue to
fail with primitive arity errors.

### Primitive semantics

- `@i64-alloc count` stack-allocates `count` `i64` elements and returns an
  `I64Vec` handle. `count` is a runtime `i64` expression. The handle lifetime
  is the enclosing `let` scope, matching `@buf-alloc` and
  `@buf-alloc-dyn`. The primitive must appear as the direct RHS of a `let`.
- `@i64-get vec index` returns element `vec[index]`. Indexing is zero-based.
- `@i64-set vec index value` stores `value` into `vec[index]` and returns 0.
- `@i64-swap vec i j` swaps `vec[i]` and `vec[j]` and returns 0. If `i == j`,
  the observable result is unchanged.
- `@i64-copy dst dst-index src src-index count` copies `count` elements from
  `src[src-index..]` to `dst[dst-index..]` and returns 0. `count` is an
  element count, not a byte count.

For all five primitives, negative counts, negative indexes, and out-of-range
access are undefined behavior in Phase 3. The caller must carry bounds
explicitly, as with byte buffers.

`@i64-copy` has overlap-safe semantics: it behaves as if the source elements
are read before the destination elements are written. This makes range shifts
safe for insertion, partitioning, and table compaction code. Codegen may use
`llvm.memmove.p0.p0.i64` or an explicit direction-aware loop.

### Type system

`I64Vec` is a new built-in, opaque handle type alongside `Buf`.

An `I64Vec` is not a `Buf`, and a `Buf` is not an `I64Vec`. This is
intentional. The typechecker should reject all of these:

- `@i64-get byte_buf 0`
- `@buf-get i64_vec 0`
- `@read 0 i64_vec n`
- `@write 1 i64_vec n`
- `@parse-i64 i64_vec off len`
- `@fmt-i64 i64_vec off value`

Programs format vector elements by reading with `@i64-get`, formatting the
returned `i64` into a byte `Buf` with `@fmt-i64`, and writing that byte
buffer.

`I64Vec` follows the same anti-escape discipline as `Buf`: it may be used
within the `let` body that owns it, but it may not be returned, stored in a
first-class value, or otherwise escape its allocation scope.

ADR 0059's hidden direct-call capture rule extends to `I64Vec` handles.
Recursive helpers may capture an outer `I64Vec` through hidden pointer
parameters exactly as they capture byte buffers today.

### Representation

`I64Vec` is not a serialization format. Its backing bytes are not observable
through `@buf-get` because the typechecker prevents treating an `I64Vec` as a
`Buf`.

The implementation may therefore use the target's native in-memory `i64`
layout. The only portable operations on an `I64Vec` are the primitives in
this ADR and later primitives that explicitly accept `I64Vec`.

### Effect integration

No new effect atom is introduced.

- `@i64-alloc` carries `{Alloc}`.
- `@i64-get` is pure (`{}`).
- `@i64-set`, `@i64-swap`, and `@i64-copy` carry `{Mut}`.

`stdlib/libc-effects.toml` is unchanged. These primitives do not cross an OS
boundary.

### Model-facing examples

The stdlib primer appendix must include at least these examples before a
paid canary:

```text
let xs = @i64-alloc 3 in
let _ = @i64-set xs 0 7 in
let _ = @i64-set xs 1 -2 in
let _ = @i64-set xs 2 10 in
@add (@i64-get xs 0) (@i64-get xs 2)
```

```text
let n = 3 in
let xs = @i64-alloc n in
rec { fill = lambda i.
  if @eq i n then 0 else
    let _ = @i64-set xs i i in
    fill (@add i 1)
} in fill 0
```

For Bundle B range tables, the model-facing convention is:

- start for row `i`: `@i64-get table (@mul i 2)`
- length for row `i`: `@i64-get table (@add (@mul i 2) 1)`

This convention does not add a new primitive in Bundle A.

### Conformance tests

Implementation must add typecheck and codegen coverage for:

- primitive signatures and arities;
- rejection of `Buf`/`I64Vec` mixups;
- allocation with dynamic count;
- get/set of positive, zero, and negative values;
- swap with distinct indexes and equal indexes;
- zero-count copy;
- cross-vector copy; and
- overlapping same-vector copy.

No canonical test vector is required. This bundle extends the `@name`
allowlist and built-in type table, but it does not add canonical node kinds.

## Alternatives considered

- **Use plain `Buf` for `@i64-get` and `@i64-set`.** Rejected. It would
  preserve byte/vector confusion, allow accidental use of input buffers as
  integer vectors, and require authors to remember `8 * count` allocation
  rules. A distinct `I64Vec` handle gives the typechecker useful leverage.
- **Keep allocation as `@buf-alloc-dyn (@mul count 8)`.** Rejected. This is
  compact for experts but fragile for generated programs. Bundle A's purpose
  is to remove representation arithmetic from model-authored solutions.
- **Add `i8` vector operations first.** Rejected. Existing `@buf-get`,
  `@buf-set`, and `@buf-copy` already cover byte-level memory. The remaining
  failure clusters need signed full-width integer storage.
- **Add generic typed vectors.** Rejected for Phase 3. A parametric
  `Vec<T>` would reopen type-system scope. `I64Vec` is narrow but directly
  matches the current `Int` representation and corpus needs.
- **Add sorting primitives in the first bundle.** Rejected. Sorting is a
  likely second bundle, but the first experiment should measure whether a
  compact integer-vector substrate improves generated programs before adding
  high-level ordering operations.
- **Give `I64Vec` a runtime length query.** Rejected. As with `Buf`, the
  program that allocates the vector already has the count and should thread it
  explicitly. A fat handle can be considered in a later library design.
- **Make `@i64-copy` overlap-undefined to mirror `@buf-copy`.** Rejected.
  Bundle A is a higher-level vector surface for generated code. Overlap-safe
  element copy avoids a common class of range-shift bugs at small
  implementation cost.

## Consequences

- Bundle A is the first library-mediated stdlib experiment target.
- Tacit gains a second buffer-like handle type, `I64Vec`, with the same
  scoped lifetime model as `Buf`.
- Codegen must carry `I64Vec` as a pointer binding and include it in hidden
  direct-call captures.
- Byte I/O remains byte-buffer based. `I64Vec` values cannot be passed to
  `@read`, `@write`, `@parse-i64`, `@fmt-i64`, or byte MEM primitives.
- Bundle B can represent start/length range tables as `I64Vec` pairs without
  adding a table type.
- Existing core-language references remain comparable; stdlib-mediated
  references must be added separately, for example as `reference.stdlib.tac`.

## Related

- [Phase 3 stdlib next steps](../plans/phase-3-stdlib-next-steps.md)
- [ADR 0047](0047-p3-stdlib-expansion-surface.md) - Phase 3 primitive
  expansion precedent
- [ADR 0059](0059-p3-rec-hidden-captures.md) - hidden direct-call captures
- [ADR 0060](0060-p3-repair-loop-outcome.md) - repair-loop outcome and next
  direction
