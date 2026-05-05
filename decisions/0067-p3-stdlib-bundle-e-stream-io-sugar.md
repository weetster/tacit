# 0067 - Phase 3 stdlib Bundle E: stream input and output sugar

**Status:** Accepted
**Date:** 2026-05-05
**Phase:** 3, library-mediated experiment, round 2
**Requires:** [ADR 0038](0038-p2-writable-buffer.md),
[ADR 0047](0047-p3-stdlib-expansion-surface.md)
**Amends:** [ADR 0047](0047-p3-stdlib-expansion-surface.md) additively.

## Context

Round 1 of the library-mediated stdlib experiment shipped four bundles
covering integer vectors, text indexing, ordering, and search/grouping
([ADR 0061](0061-p3-stdlib-bundle-a-i64-vectors.md) through
[ADR 0065](0065-p3-stdlib-bundle-c-ordering-primitives.md)). The full open
run on those bundles (`019df533-fc2a-7511-ad6f-ebdc653878ae`) confirmed the
hypothesis for tasks with stdlib references but left tasks **without**
stdlib references at ~4.25× Python generation tokens, with the strings
family highest at 6.21×.

[plans/phase-3-stdlib-round-2.md](../plans/phase-3-stdlib-round-2.md)
identifies three byte-level surface gaps driving the strings/IO ratio:
one-byte-at-a-time stdin loops, manual UTF-8 decode, and manual ASCII case
shifting. Bundle E targets the first of these and the closely related
output-side ergonomics for byte buffers.

The current `@read 0 buf cap` from ADR 0038 is the right shape for one
syscall but the wrong shape for "read all of stdin into this buffer."
Generated programs default to `let buf = @buf-alloc 1; ... loop ...`,
which is portable but adds ~25–35 tokens of boilerplate per IO/string
task. The slurp-then-process pattern is faster, simpler, and fits every
pipeline-style task in the corpus, but it is not primer-default today
because there is no single primitive that expresses it.

Bundle E should provide only stream framing and in-place byte-range
helpers. It should not perform UTF-8 decoding, character classification,
or text indexing. Those belong to Bundles F, G, and the existing Bundle B.

## Decision

Add three new `@name` primitives:

| Category | `@name` | Arity | Type | Effect | Lowering |
|----------|---------|-------|------|--------|----------|
| `STREAM-IO` | `@stdin-slurp` | 2 | `Buf -> i64 -> i64` | `{IO, Mut}` | tail-rec `@read` loop on fd 0 until EOF or `cap` reached |
| `STREAM-IO` | `@write-range` | 4 | `i64 -> Buf -> i64 -> i64 -> i64` | `{IO}` | `write(fd, buf + off, len)` |
| `BUF-MUT`   | `@buf-rev`     | 3 | `Buf -> i64 -> i64 -> i64` | `{Mut}` | in-place byte reversal of `buf[off..off+len)` |

The signatures are exact. Underapplication and overapplication continue to
fail with primitive arity errors.

### Primitive semantics

`@stdin-slurp buf cap` reads bytes from file descriptor 0 into `buf`
starting at offset 0 until end-of-file is observed or `cap` bytes have
been written, whichever comes first. The return value is the total number
of bytes written into `buf`. Short reads from the underlying syscall are
hidden: the primitive issues additional `@read` syscalls until EOF or the
cap is reached. `cap = 0` returns 0 without issuing any syscall. The cap
is caller-passed; the primitive does not inspect the size of the
underlying allocation.

`@write-range fd buf off len` writes bytes `buf[off..off+len)` to file
descriptor `fd` and returns 0. Short writes from the underlying syscall
are hidden: the primitive issues additional `write` syscalls until `len`
bytes have been emitted or a write error occurs. `len = 0` returns 0
without issuing a syscall. Negative `off`, negative `len`, and ranges that
exceed the underlying allocation are undefined behavior in Phase 3,
matching the unchecked bounds discipline for `@buf-get` and `@buf-set`.

`@buf-rev buf off len` reverses the byte order of `buf[off..off+len)` in
place and returns 0. `len = 0` and `len = 1` leave the buffer unchanged.
Negative `off`, negative `len`, and ranges that exceed the underlying
allocation are undefined behavior in Phase 3.

### Type system

`@stdin-slurp` accepts a byte `Buf` as its first argument. It does not
accept `I64Vec` or any other handle type.

`@write-range` accepts an `i64` file descriptor and a byte `Buf`. It does
not accept `I64Vec`. The fixed-size and dynamic byte buffer variants from
ADR 0038 and ADR 0047 are both usable wherever `Buf` is expected.

`@buf-rev` accepts a byte `Buf` only. Reversing an `I64Vec` is not in
scope; element-wise integer reversal is a separate primitive that the
corpus does not currently need.

The typechecker should reject all of these:

- `@stdin-slurp i64_vec cap`
- `@write-range fd i64_vec off len`
- `@buf-rev i64_vec off len`

No new built-in type, canonical node kind, type syntax, or effect atom is
introduced.

### Effect integration

- `@stdin-slurp` carries `{IO, Mut}` because it issues `read` syscalls
  and mutates the caller-provided buffer. This matches `@read` from
  ADR 0038.
- `@write-range` carries `{IO}` because it issues `write` syscalls and
  reads from but does not mutate the buffer. This matches `@write`.
- `@buf-rev` carries `{Mut}` because it mutates caller-provided storage
  and does not cross an OS boundary.

`stdlib/libc-effects.toml` is unchanged. `@stdin-slurp` and
`@write-range` are framing wrappers around the existing `@read`/`@write`
syscalls and add no new libc linkage.

### Codegen

`@stdin-slurp` lowers to a tail-recursive loop emitting one `read(0,
buf + total, cap - total)` per iteration, accumulating `total` until the
syscall returns 0 or `total == cap`. The implementation may inline the
loop or call a runtime helper, provided the observable semantics above
are preserved.

`@write-range` lowers to a tail-recursive loop emitting one `write(fd,
buf + off + written, len - written)` per iteration until `written ==
len`. As with `@stdin-slurp`, short writes are hidden.

`@buf-rev` lowers to a two-pointer in-place swap loop. Codegen may use
`memrchr`-style tricks or SIMD as long as the result equals the
byte-reverse of the input range.

### Model-facing examples

The stdlib primer appendix must include at least these examples before a
paid canary:

```text
let buf = @buf-alloc 65536 in
let n = @stdin-slurp buf 65536 in
@write-range 1 buf 0 n
```

```text
let buf = @buf-alloc 4096 in
let n = @stdin-slurp buf 4096 in
let _ = @buf-rev buf 0 n in
@write-range 1 buf 0 n
```

Examples should state the slurp-then-process pattern as the default for
stdin-driven tasks, and show `@write-range` as the way to emit a
contiguous slice of a buffer without arithmetic over `buf + off`.

### Conformance tests

Implementation must add typecheck and codegen coverage for:

- primitive signatures, arities, and effect rows;
- rejection of `Buf`/`I64Vec` mixups for all three primitives;
- `@stdin-slurp` returning 0 on empty input and on `cap = 0`;
- `@stdin-slurp` continuing across simulated short reads until EOF;
- `@stdin-slurp` stopping at `cap` even if more input is available;
- `@write-range` emitting an exact slice (off > 0 and off = 0);
- `@write-range` continuing across simulated short writes;
- `@buf-rev` on `len = 0`, `len = 1`, even and odd lengths, and
  off-aligned ranges that leave surrounding bytes intact.

No canonical test vector is required. This bundle extends the `@name`
allowlist only.

## Alternatives considered

- **Bake a default cap into `@stdin-slurp`.** Rejected. A magic byte
  budget would either be too small for some tasks or too large to
  encourage any size discipline. The caller-passed cap matches the
  existing `@read` shape and keeps the model honest about bounds.
- **Make `@stdin-slurp` allocate its own buffer and return a `Buf`.**
  Rejected. It would introduce a second allocation entry point parallel
  to `@buf-alloc-dyn` and complicate the let-RHS allocation discipline
  from ADR 0038. The two-step shape (allocate, then slurp) is shorter
  in practice and consistent with the rest of the byte surface.
- **Add `@write-all fd buf len` instead of `@write-range`.** Rejected.
  Writing a non-prefix slice is common (e.g., emit one row of a range
  table by computing `start` and `length` from a Bundle B index). The
  four-argument form covers the prefix case (`off = 0`) without adding
  a separate primitive.
- **Provide `@buf-rev-pair` or codepoint-aware reversal here.** Rejected.
  Codepoint-aware reversal belongs to Bundle G, where the encode/decode
  primitives live. `@buf-rev` is a byte-range primitive and should stay
  byte-shaped.
- **Defer all of Bundle E and rely on a primer recipe instead.**
  Rejected. The round-2 plan explicitly excludes task-shaped recipes,
  and the slurp-then-process boilerplate appears across the IO and
  strings families with the same shape. Packaging it as a primitive
  earns its primer tokens on the net-token rule.

## Consequences

- The slurp-then-process pattern becomes primer-default for stdin-driven
  tasks. Generated programs no longer need a one-byte tail-rec loop to
  read input.
- `@write-range` standardizes slice output without arithmetic over
  buffer addresses.
- `@buf-rev` enables byte-level reverse tasks (`echo-reverse`, simple
  `reverse-string` on ASCII input) without manual two-pointer code.
- Codegen carries two new IO-flavored primitives. They are framing
  wrappers around existing syscalls and do not add a new OS-boundary
  ABI.
- Library-mediated results still need separate reporting and do not
  satisfy primer-only Phase 3 gates.

## Related

- [Phase 3 stdlib round 2 plan](../plans/phase-3-stdlib-round-2.md)
- [ADR 0038](0038-p2-writable-buffer.md) - `@read`/`@write` and buffer
  handles
- [ADR 0047](0047-p3-stdlib-expansion-surface.md) - Phase 3 primitive
  expansion precedent
- [ADR 0068](0068-p3-stdlib-bundle-f-ascii-classification.md) - Bundle F
  ASCII classification
- [ADR 0069](0069-p3-stdlib-bundle-g-utf8-codepoints.md) - Bundle G
  UTF-8 codepoint primitives
