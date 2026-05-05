# 0069 - Phase 3 stdlib Bundle G: UTF-8 codepoint decode and encode

**Status:** Accepted
**Date:** 2026-05-05
**Phase:** 3, library-mediated experiment, round 2
**Requires:** [ADR 0038](0038-p2-writable-buffer.md),
[ADR 0047](0047-p3-stdlib-expansion-surface.md)
**Amends:** [ADR 0047](0047-p3-stdlib-expansion-surface.md) additively.

## Context

Round 2 of the library-mediated stdlib experiment targets the byte-level
surface gaps identified in
[plans/phase-3-stdlib-round-2.md](../plans/phase-3-stdlib-round-2.md).
The largest single per-task token contributor in the strings family is
manual UTF-8 decoding. `is-palindrome` and `reverse-string` each branch
on `@lt b 128`, `@lt b 224`, `@lt b 240` and reconstruct codepoints by
hand, costing ~80–150 tokens per task. `longest-word` packs captured
codepoints in a base-27 representation rather than re-encoding them as
UTF-8 bytes.

These patterns have direct analogues in mainstream language standard
libraries: Python's `str.encode`/`bytes.decode`, Rust's `char::len_utf8`
and `str::from_utf8`, Go's `utf8.DecodeRune`/`utf8.EncodeRune`. Phase 3
already commits to UTF-8 as the source-text encoding ([ADR 0013](0013-canonical-text-format-frozen.md)).

Bundle G should provide only codepoint-level decode, encode, and length
inspection. It should not classify codepoints, fold case, or normalize.
Those are Unicode-database operations that Tacit-Lite has no scope to
ship.

## Decision

Add three new `@name` primitives:

| Category | `@name` | Arity | Type | Effect | Lowering |
|----------|---------|-------|------|--------|----------|
| `UTF8` | `@utf8-decode` | 2 | `Buf -> i64 -> i64` | `{}` | decode one codepoint, return packed `cp * 8 + byte_len` |
| `UTF8` | `@utf8-encode` | 3 | `Buf -> i64 -> i64 -> i64` | `{Mut}` | encode one codepoint, return bytes written (1..=4) |
| `UTF8` | `@utf8-len`    | 1 | `i64 -> i64` | `{}` | encoded byte length of a codepoint (1..=4), or 0 if invalid |

The signatures are exact. Underapplication and overapplication continue
to fail with primitive arity errors.

### Primitive semantics

`@utf8-decode buf off` reads one UTF-8 codepoint starting at byte
offset `off` of `buf` and returns the packed value `cp * 8 + byte_len`,
where `cp` is the codepoint scalar value (0..=0x10FFFF) and `byte_len`
is the number of bytes consumed (1, 2, 3, or 4).

If the byte sequence at `buf[off..]` is not a valid UTF-8 codepoint
under RFC 3629, the primitive returns 0. This single sentinel covers
all malformed cases: lone continuation bytes, truncated multi-byte
sequences (caller-bounded; see below), overlong encodings, and
surrogate codepoints (0xD800..=0xDFFF). Callers detect malformed input
by checking `byte_len == 0` after unpacking, computed as
`@mod packed 8`.

The primitive does not consult an explicit length argument. The caller
must ensure that at least one byte is readable at `off` and that any
continuation bytes implied by the lead byte are also within the
allocated range; reads past the underlying allocation are undefined
behavior, matching the unchecked bounds discipline for `@buf-get`.

`@utf8-encode buf off cp` writes the UTF-8 encoding of `cp` to
`buf[off..off+n)` for `n` in 1..=4 and returns `n`. Invalid codepoints
(`cp < 0`, `cp > 0x10FFFF`, or surrogate codepoints in
0xD800..=0xDFFF) cause the primitive to return 0 without writing any
bytes. Reads or writes past the underlying allocation are undefined
behavior.

`@utf8-len cp` returns the number of UTF-8 bytes that `cp` would
encode to (1, 2, 3, or 4) for valid codepoints, and returns 0 for
invalid codepoints. The primitive does not touch memory.

### Packing convention

The decode return value packs two outputs into one `i64` as
`cp * 8 + byte_len`. The 3-bit length field is sufficient because
`byte_len` is always in 0..=4. Callers unpack as:

- `cp = @div packed 8`
- `byte_len = @mod packed 8`

This is the tightest meaningful packing for the (codepoint, length)
pair. The wider round-1 packing factors (`* 40000`, `* 80000`) are not
needed because `byte_len` cannot exceed three bits. The convention
follows the round-1 rule: small packed pair → packed `i64`; greater
than two outputs → write to a caller-supplied buffer.

### Type system

`@utf8-decode` and `@utf8-encode` accept a byte `Buf` only. They do
not accept `I64Vec`. The fixed-size and dynamic byte buffer variants
from ADR 0038 and ADR 0047 are both usable wherever `Buf` is
expected.

`@utf8-len` accepts an `i64` codepoint. It does not touch memory.

The typechecker should reject all of these:

- `@utf8-decode i64_vec off`
- `@utf8-encode i64_vec off cp`
- `@utf8-len buf` (a `Buf` is not an `i64`)

No new built-in type, canonical node kind, type syntax, or effect atom
is introduced.

### Effect integration

- `@utf8-decode` is pure (`{}`). It reads from a buffer but does not
  mutate it, matching `@buf-get` and `@i64-get`.
- `@utf8-encode` carries `{Mut}` because it writes bytes into the
  caller-provided buffer.
- `@utf8-len` is pure (`{}`).

`stdlib/libc-effects.toml` is unchanged. The primitives lower to
straight-line bit arithmetic and small branches; they do not link
against ICU, libutf8proc, or any external library and do not consult
locale data.

### Codegen

`@utf8-decode` lowers to a four-arm branch on the lead byte's high
bits, with continuation-byte validation per RFC 3629:

- `0xxxxxxx` → 1-byte ASCII, no further reads;
- `110xxxxx 10xxxxxx` → 2-byte, codepoint must be ≥ 0x80;
- `1110xxxx 10xxxxxx 10xxxxxx` → 3-byte, codepoint must be ≥ 0x800
  and not in the surrogate range 0xD800..=0xDFFF;
- `11110xxx 10xxxxxx 10xxxxxx 10xxxxxx` → 4-byte, codepoint must be
  ≥ 0x10000 and ≤ 0x10FFFF.

Continuation bytes that don't match `10xxxxxx`, lead bytes outside
the four shapes above, overlong encodings, surrogate codepoints, and
codepoints above 0x10FFFF all collapse to the single sentinel return
value of 0.

`@utf8-encode` lowers to a four-arm branch on `cp` against the
boundary values 0x80, 0x800, and 0x10000, emitting the lead byte and
continuation bytes for each width.

`@utf8-len` lowers to a three-comparison branch ladder.

The implementation may use a precomputed lead-byte length table
(`utf8_lead_len[256]`) as long as the observable semantics above are
preserved.

### Model-facing examples

The stdlib primer appendix must include at least these examples before
a paid canary:

```text
let buf = @buf-alloc 64 in
let n = @stdin-slurp buf 64 in
let packed = @utf8-decode buf 0 in
let cp = @div packed 8 in
let len = @mod packed 8 in
@add cp len
```

```text
let out = @buf-alloc 4 in
let n = @utf8-encode out 0 0x1F600 in
@write-range 1 out 0 n
```

```text
@utf8-len 0x4E2D
```

Examples should state the packing convention, the malformed-input
sentinel, and that `@utf8-encode` rejects invalid codepoints by
returning 0 without writing.

The primer must also note the round-2 idiom for codepoint-aware text
processing: slurp with `@stdin-slurp`, advance offset by the
`byte_len` returned from each `@utf8-decode`, and emit codepoints with
`@utf8-encode` to a caller-allocated output buffer.

### Conformance tests

Implementation must add typecheck and codegen coverage for:

- primitive signatures, arities, and effect rows;
- rejection of `Buf`/`I64Vec` mixups;
- `@utf8-decode` on each of the four widths (e.g., U+0041, U+00E9,
  U+4E2D, U+1F600);
- `@utf8-decode` returning 0 for: lone continuation byte 0x80,
  overlong encoding of U+0000 as `0xC0 0x80`, truncated 4-byte
  prefix `0xF0 0x9F`, surrogate `0xED 0xA0 0x80`, codepoint
  0x110000 attempt;
- `@utf8-encode` round-tripping each of the four widths;
- `@utf8-encode` returning 0 without writing for `cp = -1`,
  `cp = 0xD800`, and `cp = 0x110000`;
- `@utf8-len` agreeing with `@utf8-encode` on all valid codepoints
  tested; and
- `@utf8-len` returning 0 for invalid codepoints.

No canonical test vector is required. This bundle extends the `@name`
allowlist only.

## Alternatives considered

- **Return `(cp, byte_len)` via an out-buffer instead of packing.**
  Rejected. A caller-supplied `I64Vec` slot or pair of `@buf-set`
  calls is more boilerplate than the packed integer. The (codepoint,
  small-length) pair fits cleanly into one `i64`, and the round-1
  packing precedent already uses this convention.
- **Use `cp * 16 + byte_len` or `cp * 256 + byte_len`.** Rejected.
  Three bits are sufficient for `byte_len in 0..=4`. Tighter packing
  keeps the codepoint shift small enough that a caller composing
  decode-then-arithmetic does not need to worry about overflow at
  the high end of Unicode (`0x10FFFF * 8 = 0x87FFF8`, well below
  `i64::MAX`).
- **Provide separate `@utf8-decode-len` and `@utf8-decode-cp`
  primitives.** Rejected. Two primitives would duplicate the
  validation logic and force a redundant scan of the byte sequence.
  A single packed-return primitive is cheaper and matches Bundle B's
  start/length packing precedent.
- **Have `@utf8-decode` accept and respect a `cap` argument** to
  bound continuation reads. Rejected. The unchecked-bounds
  discipline used everywhere else in the byte surface keeps the
  primitive call shorter, and callers already know they must not
  decode past their slurped length. A cap-aware variant can be added
  later if the corpus shows it pays for itself.
- **Add `@utf8-is-valid buf off len`** to validate a byte range
  without decoding. Rejected for round 2. The corpus doesn't have
  validation-only tasks; programs that need validity composition can
  call `@utf8-decode` in a loop and check for the 0 sentinel.
- **Add Unicode case folding, normalization, or character-property
  primitives.** Rejected. Those require shipping a Unicode database
  (~100KB minimum) and are outside Tacit-Lite scope. Bundle F covers
  the ASCII-range case shifts; full Unicode behavior is a Phase 4+
  question.
- **Use `null` or `-1` as the malformed sentinel.** Rejected. Tacit
  has no null. `-1` would force callers to check for negative
  values, which complicates the unpacking arithmetic. Returning 0
  with `byte_len = 0` is naturally distinguishable from any valid
  decode (which always has `byte_len >= 1`).

## Consequences

- Generated programs that need codepoint awareness can decode and
  encode UTF-8 in single primitive calls instead of open-coding the
  1/2/3/4-byte branches.
- The packed-return convention from round 1 extends cleanly to a
  third bundle, with the smallest meaningful packing factor used so
  far.
- Bundle G does not preempt Bundle F. Programs that operate on ASCII
  bytes continue to use `@buf-get` and the Bundle F primitives;
  Bundle G is for tasks that need to count or reconstruct codepoints.
- Library-mediated results still need separate reporting and do not
  satisfy primer-only Phase 3 gates.

## Related

- [Phase 3 stdlib round 2 plan](../plans/phase-3-stdlib-round-2.md)
- [ADR 0013](0013-canonical-text-format-frozen.md) - UTF-8 as source
  text encoding
- [ADR 0047](0047-p3-stdlib-expansion-surface.md) - Phase 3 primitive
  expansion precedent
- [ADR 0062](0062-p3-stdlib-bundle-b-text-indexing.md) - range-table
  packing precedent
- [ADR 0067](0067-p3-stdlib-bundle-e-stream-io-sugar.md) - Bundle E
  stream IO sugar
- [ADR 0068](0068-p3-stdlib-bundle-f-ascii-classification.md) -
  Bundle F ASCII classification
