# 0068 - Phase 3 stdlib Bundle F: ASCII classification and case

**Status:** Accepted
**Date:** 2026-05-05
**Phase:** 3, library-mediated experiment, round 2
**Requires:** [ADR 0047](0047-p3-stdlib-expansion-surface.md)
**Amends:** [ADR 0047](0047-p3-stdlib-expansion-surface.md) additively.

## Context

Round 2 of the library-mediated stdlib experiment targets the byte-level
surface gaps identified in
[plans/phase-3-stdlib-round-2.md](../plans/phase-3-stdlib-round-2.md).
Generated programs in the strings family repeatedly open-code two
patterns:

- ASCII case shifting as inline conditionals,
  `if @ge byte 97 then if @le byte 122 then @sub byte 32 else byte else
  byte`, appearing in `title-case` and elsewhere; and
- Character classification as multi-branch byte-equality chains,
  e.g. `count-vowels` enumerating ten branches for the lowercase and
  uppercase vowels, or `valid-sudoku-row` checking digits with two-bound
  inequality.

These patterns each cost 5–25 tokens per occurrence, recur across many
tasks, and have direct analogues in mainstream language standard
libraries (`tolower`, `isalpha`, `isdigit`, `isspace`).

Bundle F should provide only ASCII-range classification and case shift.
It should not handle Unicode case folding, locale-aware classification,
or multi-byte encoding. UTF-8 codepoint awareness belongs to Bundle G.

## Decision

Add five new `@name` primitives:

| Category | `@name` | Arity | Type | Effect | Lowering |
|----------|---------|-------|------|--------|----------|
| `ASCII-CASE`  | `@ascii-tolower`   | 1 | `i64 -> i64` | `{}` | byte +32 if 65..=90, else identity |
| `ASCII-CASE`  | `@ascii-toupper`   | 1 | `i64 -> i64` | `{}` | byte -32 if 97..=122, else identity |
| `ASCII-CLASS` | `@ascii-is-alpha`  | 1 | `i64 -> i64` | `{}` | 1 if 65..=90 or 97..=122, else 0 |
| `ASCII-CLASS` | `@ascii-is-digit`  | 1 | `i64 -> i64` | `{}` | 1 if 48..=57, else 0 |
| `ASCII-CLASS` | `@ascii-is-space`  | 1 | `i64 -> i64` | `{}` | 1 if 9, 10, 11, 12, 13, or 32, else 0 |

The signatures are exact. Underapplication and overapplication continue
to fail with primitive arity errors.

### Primitive semantics

`@ascii-tolower b` returns `b + 32` if `b` is in the inclusive range
65..=90 (uppercase A through Z), and returns `b` unchanged otherwise.
The argument is treated as a signed `i64`; values outside 0..=127 are
returned unchanged.

`@ascii-toupper b` returns `b - 32` if `b` is in the inclusive range
97..=122 (lowercase a through z), and returns `b` unchanged otherwise.
Values outside 0..=127 are returned unchanged.

`@ascii-is-alpha b` returns 1 if `b` is in 65..=90 or 97..=122, and
returns 0 otherwise. Values outside 0..=127 return 0.

`@ascii-is-digit b` returns 1 if `b` is in 48..=57 (ASCII digits 0
through 9), and returns 0 otherwise.

`@ascii-is-space b` returns 1 if `b` is one of: 9 (TAB), 10 (LF), 11
(VT), 12 (FF), 13 (CR), or 32 (SP); and returns 0 otherwise. The set
matches C's `isspace` for the C/POSIX locale.

All five primitives are total functions over `i64`. They never read
memory, never mutate state, never trap, and never depend on locale or
runtime configuration.

### Type system

All five primitives accept and return `i64`. They do not take a `Buf`
or `I64Vec`. The caller is responsible for loading the byte to
classify, typically with `@buf-get`.

No new built-in type, canonical node kind, type syntax, effect atom, or
libc signature is introduced.

### Effect integration

All five primitives are pure (`{}`). They do not allocate, do not
mutate, and do not perform I/O.

`stdlib/libc-effects.toml` is unchanged. The primitives lower to
straight-line arithmetic and comparisons; they do not link against C
`ctype.h` and do not consult locale data.

### Codegen

Each primitive lowers to a small constant-shape sequence of `icmp` and
arithmetic. None requires a function call, table lookup, or branch on
locale. The implementation should not call `tolower`, `toupper`, or any
`is*` from libc, both to avoid locale dependence and to keep the
primitives pure under the effect system.

The five primitives are eligible for inline expansion at every call
site. They are not eligible for hoisting across `@buf-set` calls
because they read no memory at all; ordering is unconstrained.

### Model-facing examples

The stdlib primer appendix must include at least these examples before
a paid canary:

```text
@ascii-tolower 65
```

```text
let buf = @buf-alloc 1 in
let _ = @buf-set buf 0 (@ascii-toupper (@buf-get buf 0)) in
@buf-get buf 0
```

```text
let b = @buf-get buf i in
if @eq (@ascii-is-space b) 1 then 0 else 1
```

Examples should state that the primitives are ASCII-range only, that
non-ASCII bytes pass through case shifts unchanged, and that
classification primitives return 0 or 1 (not a Boolean type, since
Tacit-Lite has none).

The primer must also call out that `@ascii-is-vowel` does not exist:
the corpus task that needs vowel detection composes `@ascii-tolower`
with a five-branch equality check, mirroring the ADR 0066 stance
against corpus-shaped naming.

### Conformance tests

Implementation must add typecheck and codegen coverage for:

- primitive signatures, arities, and `{}` effect;
- `@ascii-tolower` and `@ascii-toupper` boundary behavior at 64, 65,
  90, 91, 96, 97, 122, 123;
- case shifts leaving 0, 32, 64, 91, 123, 127, 128, 255, and negative
  inputs unchanged;
- `@ascii-is-alpha` returning 1 only for 65..=90 and 97..=122;
- `@ascii-is-digit` returning 1 only for 48..=57;
- `@ascii-is-space` returning 1 only for the six bytes 9, 10, 11, 12,
  13, 32; and
- `@ascii-is-*` returning 0 for negative inputs and for inputs above
  127.

No canonical test vector is required. This bundle extends the `@name`
allowlist only.

## Alternatives considered

- **Add `@ascii-is-vowel`.** Rejected. The operation does not appear in
  any mainstream language standard library and shows up in essentially
  one corpus task (`count-vowels`). The task composes `@ascii-tolower`
  with a five-branch equality check; that composition is short and
  generic. Adding `@ascii-is-vowel` would establish a precedent for
  corpus-shaped naming that round 1 explicitly avoided.
- **Add `@ascii-is-alnum`, `@ascii-is-punct`, `@ascii-is-print`,
  `@ascii-is-cntrl`, etc.** Rejected for round 2. The corpus does not
  exercise these classes in a way that justifies their primer cost
  under the round-2 net-token rule. They can be added later if a
  follow-up evaluation demonstrates demand.
- **Make classification primitives return a `Bool`-like type.**
  Rejected. Tacit-Lite has no Boolean type; numeric 0/1 matches the
  existing convention used by `@eq`, `@lt`, `@ge`, etc.
- **Use libc `tolower`/`toupper`/`isalpha`/`isdigit`/`isspace`.**
  Rejected. Locale-dependent semantics would force an `IO`-flavored
  effect or break the purity claim. Inline arithmetic is shorter,
  faster, and locale-stable.
- **Apply case shifts to UTF-8 codepoints rather than bytes.**
  Rejected. Unicode case folding is out of scope for Tacit-Lite. The
  byte-level shift covers the corpus's ASCII tasks, and Bundle G
  provides codepoint awareness for tasks that need it without
  bundling case folding.
- **Combine all five primitives into one `@ascii-class b`-style
  predicate selector with a class argument.** Rejected. A single
  selector would require an integer encoding for class identity,
  which is opaque without primer support, and would defeat the
  inlining win because each call would carry a runtime branch on the
  class argument.

## Consequences

- ASCII case shifting and classification become single-call
  operations. Generated programs no longer need inline conditionals
  for `tolower`, `toupper`, `isalpha`, `isdigit`, or `isspace`.
- The five primitives are pure, locale-stable, and inline at every
  call site, so they compose freely with arithmetic and control flow.
- Bundle F does not preempt Bundle G. Programs that need codepoint
  awareness use Bundle G primitives and can still call Bundle F on
  ASCII bytes that fall in the codepoint range 0..=127.
- Library-mediated results still need separate reporting and do not
  satisfy primer-only Phase 3 gates.

## Related

- [Phase 3 stdlib round 2 plan](../plans/phase-3-stdlib-round-2.md)
- [ADR 0047](0047-p3-stdlib-expansion-surface.md) - Phase 3 primitive
  expansion precedent
- [ADR 0066](0066-p3-cross-family-tier-matching.md) - against
  corpus-shaped naming
- [ADR 0067](0067-p3-stdlib-bundle-e-stream-io-sugar.md) - Bundle E
  stream IO sugar
- [ADR 0069](0069-p3-stdlib-bundle-g-utf8-codepoints.md) - Bundle G
  UTF-8 codepoint primitives
