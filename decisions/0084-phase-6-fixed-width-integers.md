# 0084 - Phase 6 fixed-width integers and bit primitives

Date: 2026-05-16

## Status

Accepted

## Context

Phase 6 Stage 6 needs enough numeric surface for systems-style Tacit code:
register values, opcodes, bit fields, byte-order assembly, and deterministic
overflow behavior. The existing `Int` surface is a legacy signed `i64`
computational type with arithmetic primitives inherited from Phase 1. That is
not precise enough for CPU-core and decoder examples.

The Stage 6 surface must not add untyped pointers, unchecked memory access, or
arbitrary host escape hatches. It also must preserve the existing canonical
format discipline: additive evolution is acceptable, but a new canonical node
kind is not necessary for integer widths.

## Decision

Tacit adds first-class fixed-width integer types named in type position by
ordinary canonical symbols:

- Signed: `i8`, `i16`, `i32`, `i64`
- Unsigned: `u8`, `u16`, `u32`, `u64`

Canonical representation uses `(sym i8)`, `(sym u8)`, and so on. No new AST tag
is introduced. `Int` remains a compatibility type for existing programs and is
accepted as the legacy signed 64-bit scalar at old primitive boundaries.

Integer literals are initially untyped. In an expected fixed-width type
position, a literal may default to that width only when its decimal value fits
the target type. Truncating or wrapping a literal requires an explicit cast.
Without a fixed-width expectation, literals default to legacy `Int`.

Fixed-width primitives are compiler-recognized pure primitives. Source-level
stdlib wrappers may later wrap them, but Stage 6 keeps the low-level operations
in the compiler so codegen can lower them directly and diagnostics can name the
exact width/sign policy.

Primitive naming is type-prefixed and explicit:

- Casts:
  - `@<dst>-from-int-wrap`
  - `@<src>-to-<dst>-trunc`
  - `@<src>-to-<dst>-sext`
  - `@<src>-to-<dst>-zext`
- Wrapping arithmetic:
  - `@<ty>-add-wrap`, `@<ty>-sub-wrap`, `@<ty>-mul-wrap`
- Checked arithmetic:
  - `@<ty>-add-check`, `@<ty>-sub-check`
  - result type `{ok: Bool, value: <ty>}`
- Saturating arithmetic:
  - `@<ty>-add-sat`, `@<ty>-sub-sat`
- Bit operations:
  - `@<ty>-and`, `@<ty>-or`, `@<ty>-xor`, `@<ty>-not`
- Shifts and rotates:
  - `@<ty>-shl`, `@<ty>-shr`, `@<ty>-rotl`, `@<ty>-rotr`
  - shifts reject statically known negative or too-wide literal counts; dynamic
    out-of-range shifts evaluate deterministically instead of relying on LLVM
    undefined behavior.
  - rotates use count modulo the integer width.
- Masks:
  - `@<ty>-mask-low n` returns the value with the low `n` bits set, clamped to
    the type width.
- Byte-order helpers:
  - `@u16-from-be`, `@u16-from-le`
  - `@u32-from-be`, `@u32-from-le`
  - `@u64-from-be`, `@u64-from-le`
  - `@u16-bswap`, `@u32-bswap`, `@u64-bswap`

Signed fixed-width values are represented canonically at runtime as
sign-extended `i64` values. Unsigned fixed-width values are represented as
zero-extended `i64` values. This keeps Stage 6 compatible with the existing
single-scalar codegen path while preserving fixed-width semantics at primitive
boundaries.

## Consequences

- Existing `Int` programs continue to typecheck and compile.
- Fixed-width operations cannot silently mix signedness or widths; the
  typechecker requires matching fixed-width operands.
- Checked arithmetic exposes success/failure as an ordinary record and can be
  projected with `.ok` and `.value`.
- Byte-order helpers are pure compiler primitives for Stage 6. Stage 9 may
  wrap them in source-level stdlib packages without changing their semantics.
- The parser and emitter do not need a new canonical form. Authoring and
  inspection views render the new type symbols and primitive symbols through
  the existing symbol paths.

## Rejected Alternatives

Adding a new canonical integer-type node was rejected. Width and signedness are
type names, and the existing `sym` type-position representation already gives
canonical identity without expanding the core AST.

Making old `@add`, `@sub`, and ordered comparison primitives overloaded for all
integer types was rejected. It would hide signedness policy at the most
important systems boundaries. Fixed-width code should spell the chosen
behavior. Equality remains available for matching fixed-width values because it
does not choose an overflow or ordering policy.

Returning packed integers from checked arithmetic was rejected. A record result
keeps success/failure explicit and fits the Phase 4 structural product surface.
