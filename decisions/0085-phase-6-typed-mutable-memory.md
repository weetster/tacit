# 0085 - Phase 6 typed mutable memory

**Status:** Accepted
**Date:** 2026-05-16
**Phase:** 6, Stage 7
**Closes:** [phase-6-plan.md Q-P6-9](../plans/phase-6-plan.md)
**Amends:** [ADR 0038](0038-p2-writable-buffer.md),
[ADR 0061](0061-p3-stdlib-bundle-a-i64-vectors.md), and
[ADR 0073](0073-p4-function-values-and-closures.md) additively.

## Context

Phase 6 Stage 7 adds the typed mutable-memory surface that systems-style Tacit
programs need: CPU register files, memory buses, instruction-decode tables,
and any other shape where bytes or words are loaded and stored in a typed,
bounded region.

The existing surface has two ad hoc handle types: `Buf` (byte buffer,
ADR 0038, ADR 0047) and `I64Vec` (i64 vector, ADR 0061). Both are stack-
allocated, anti-escape, and useful but limited in three ways that block
emulator-style code generation:

1. **Length is not carried by the handle.** Every primitive that reads or
   writes a buffer also threads a separate `Int` length argument. Generated
   programs lose track of which `n` belongs to which buffer.
2. **Out-of-range access is undefined behavior.** Off-by-one errors corrupt
   memory silently rather than producing a deterministic, actionable trap.
3. **Only two element widths exist.** Reading a u32 little-endian out of a
   byte buffer requires four `@buf-get` calls plus three `@shl` plus three
   `@or`. CPU-state shapes that want native widths must hand-pack into
   `I64Vec` or into bytes.

Stage 6 (ADR 0084) added fixed-width integer types `i8`/`u8`/.../`i64`/`u64`
and operations on them. Stage 7 adds the matching typed mutable-memory
surface so a value of width `W` can be loaded from and stored into storage of
that width.

The Stage 7 surface must not:

- introduce arbitrary `extern "C"` declarations,
- introduce untyped pointer escapes,
- offer unchecked memory access by default,
- introduce row polymorphism, capabilities, refinement types, or a borrow
  checker,
- depend on the host-interface ABI (Stage 10 owns that boundary).

## Decision

### New types

Eight typed-vector handle types, one per Stage 6 integer width:

| Name      | Element |
|-----------|---------|
| `i8vec`   | `i8`    |
| `u8vec`   | `u8`    |
| `i16vec`  | `i16`   |
| `u16vec`  | `u16`   |
| `i32vec`  | `i32`   |
| `u32vec`  | `u32`   |
| `i64vec`  | `i64`   |
| `u64vec`  | `u64`   |

Type names are lowercase and parallel to the Stage 6 integer type names.
Canonical representation uses `(sym u8vec)`, `(sym i32vec)`, etc. No new AST
tag is introduced, matching the Stage 6 strategy for fixed-width integer
type names.

There is no type-level length parameter. Length lives in the runtime handle
and is queried with `@<ty>vec-len`. This is a deliberate departure from the
Phase 2 `Buf N` design and removes the integer-in-type-position machinery
that has otherwise been unused since Phase 2.

A vec handle is non-escapable. The handle may be passed to vec primitives,
to direct-call `rec` helpers (ADR 0059 hidden captures), and let-bound
within the allocating function. It may not be captured by a first-class
closure, stored in a record, returned from a function, or otherwise stored
in a first-class value position. This generalizes the anti-escape rule that
already applies to `Buf` and `I64Vec` (ADR 0038, ADR 0061, ADR 0073).

### Uniform primitive surface

Every typed vector exposes the same four primitives:

| Primitive          | Signature                            | Effect    |
|--------------------|--------------------------------------|-----------|
| `@<ty>vec-alloc`   | `Int -> <ty>vec`                     | `{Alloc}` |
| `@<ty>vec-len`     | `<ty>vec -> Int`                     | `{}`      |
| `@<ty>vec-get`     | `<ty>vec -> Int -> <ty>`             | `{}`      |
| `@<ty>vec-set`     | `<ty>vec -> Int -> <ty> -> Int`      | `{Mut}`   |

`<ty>` is one of the eight integer widths; `<ty>vec` is the matching
typed-vector type. `set` returns `Int` `0`, matching existing buffer-set
conventions. Like `@buf-alloc`, `@<ty>vec-alloc` must appear as the direct
right-hand side of a `let` binding so codegen can lower it as a stack
allocation in the enclosing function frame.

### `u8vec` extras

Byte buffers carry I/O, decoding, and slicing work. `u8vec` exposes five
additional primitives:

| Primitive          | Signature                                                         | Effect  |
|--------------------|-------------------------------------------------------------------|---------|
| `@u8vec-fill`      | `u8vec -> Int -> Int -> u8 -> Int`                                | `{Mut}` |
| `@u8vec-copy`      | `u8vec -> Int -> u8vec -> Int -> Int -> Int`                      | `{Mut}` |
| `@u8vec-slice`     | `u8vec -> Int -> Int -> u8vec`                                    | `{}`    |
| `@u8vec-eq`        | `u8vec -> Int -> u8vec -> Int -> Int -> Bool`                     | `{}`    |
| `@u8vec-scan`      | `u8vec -> Int -> Int -> u8 -> Int`                                | `{}`    |

`@u8vec-copy` is overlap-safe (LLVM `memmove`), matching `@i64-copy`.
`@u8vec-slice off len` returns a sub-handle that shares storage with the
parent. The sub-handle is itself a non-escapable `u8vec` with the sub-range
length reported by `@u8vec-len`. Multiple slices into the same parent may
alias and observe each other's writes; this is documented behavior, not a
borrow violation. Slices inherit the parent's anti-escape boundary because
they are `u8vec` values.

### Byte-bus cross-width helpers

A byte buffer used as a memory bus needs typed multi-byte loads and stores.
`u8vec` exposes twelve helpers for u16/u32/u64 in little- and big-endian
flavors:

| Load                       | Store                       |
|----------------------------|-----------------------------|
| `@u8vec-load-u16-le v off` | `@u8vec-store-u16-le v off x` |
| `@u8vec-load-u16-be v off` | `@u8vec-store-u16-be v off x` |
| `@u8vec-load-u32-le v off` | `@u8vec-store-u32-le v off x` |
| `@u8vec-load-u32-be v off` | `@u8vec-store-u32-be v off x` |
| `@u8vec-load-u64-le v off` | `@u8vec-store-u64-le v off x` |
| `@u8vec-load-u64-be v off` | `@u8vec-store-u64-be v off x` |

Loads are pure; stores carry `{Mut}`. Loads read `width / 8` consecutive
bytes starting at `off`, assemble them by the requested endianness, and
return a fixed-width unsigned value. Stores accept a fixed-width unsigned
value, decompose it by the requested endianness, and write `width / 8`
consecutive bytes starting at `off`.

These helpers are compiler primitives rather than source-level wrappers
because they participate in the bounds check (one check covering the whole
multi-byte access) and because they are the central memory-bus idiom for
emulator-style examples.

### Bounds semantics

Every `get`, `set`, `fill`, `copy`, `slice`, `eq`, `scan`, and byte-bus
load or store performs a runtime bounds check against the handle's length.
A bounds violation invokes `llvm.trap`, which deterministically aborts the
process. Out-of-range access is **not** undefined behavior under Stage 7.

The trap is not represented as an effect atom. `get` remains pure, `set`
remains `{Mut}`. The bounds policy is documented as part of the primitive
semantics, matching Stage 6's stance on integer overflow (where overflow is
encoded in the operation name or result shape rather than in the effect
lattice). Phase 8 may elide statically-provable safe accesses; Phase 6 only
guarantees that an out-of-range access traps cleanly rather than corrupting
memory.

No unchecked variant is exposed. "No unsafe unchecked memory access by
default" (ADR 0079) is honored at the primitive level.

### Allocation model

All `@<ty>vec-alloc count` calls stack-allocate at the enclosing function
entry, lowered through LLVM `alloca`. The handle lifetime ends at the
allocating function's return. There is no heap allocation, no manual
deallocation, and no static-vs-dynamic size split: every call takes a
runtime `Int` count.

Heap-owned vectors and host-provided buffers are Stage 10 (host-interface
ABI) work. The host owns memory; Tacit operates on handles the host hands
in.

### Coexistence with `Buf` and `I64Vec`

`Buf`, `I64Vec`, and their existing primitives are unchanged. Phase 1
through Phase 5 examples continue to compile and run.

The Stage 7 primer surface teaches only the new typed-vec family. Old
surface is documented as legacy and accepted for backward compatibility.
`Buf` is not aliased to `u8vec` because `Buf` is threaded through the
existing libc primitives (`@read`, `@write`, `@parse-i64`, `@fmt-i64`) with
explicit `Int` length arguments and a different anti-escape implementation;
aliasing would change those signatures. Stage 9 (source-level stdlib) may
wrap legacy I/O primitives to consume `u8vec` if desired.

### Diagnostics

| Kind                       | Producer  | Meaning |
|----------------------------|-----------|---------|
| `escape-vec-handle`        | typecheck | a typed-vec handle escapes its `let` scope. Generalizes the existing `invalid-capture` for the eight new types. |
| `vec-type-mismatch`        | typecheck | a vec primitive received the wrong vec type (for example `@u32vec-get` applied to a `u8vec`). |
| `vec-alloc-not-in-let`     | codegen   | `@<ty>vec-alloc` appeared outside a direct `let` right-hand side. |
| `bounds-violation`         | runtime   | bounds-check trap fired. Stage 7 emits a fixed-format process abort; richer trap diagnostics are deferred to Phase 8. |

## Consequences

- Tacit gains eight typed mutable-memory handle types, all length-carrying
  and bounds-checked.
- Generated code no longer threads a separate length argument alongside
  each handle, which removes a known source of off-by-one mistakes in
  AI-generated programs.
- Byte-bus typed loads and stores replace hand-rolled byte-shift chains.
- `Buf` and `I64Vec` continue to work; their tests and primer references
  are not affected.
- The fixed Tacit-Lite effect lattice is unchanged. No `Trap` atom is
  introduced.
- The constrained host-interface ABI (Stage 10) inherits a clean typed-
  memory surface to expose at the boundary.

## Rejected alternatives

- **Generic `Vec<T>`.** Rejected. Parametric vector types would require
  generic primitive lookups and a richer surface than Phase 6 needs. The
  eight explicit names compose with the eight Stage 6 integer types
  one-for-one and match the established `@<ty>-<op>` naming pattern.
- **Type-level length parameter (`u8vec N`).** Rejected. Phase 2's
  `Buf N` has not seen meaningful use and adds canonical complexity for
  little gain. Storing length in the handle keeps the type surface flat.
- **Aliasing `Buf` to `u8vec`.** Rejected. The libc-flavored primitives
  that consume `Buf` thread `Int` lengths explicitly and rely on bare
  pointer semantics. Reusing the name for a length-bearing handle would
  change their effective signatures and break Phase 2/3 examples.
- **Returning `{ok, value}` from `get`.** Rejected. Universal record
  destructuring at every access point bloats program text. A trap is the
  right behavior for a logic bug.
- **Adding a `Trap` effect atom.** Rejected. Stage 6 handled overflow
  without expanding the lattice. Bounds violation is the same kind of
  logic-bug abort and follows the same policy.
- **`@<ty>vec-swap` for all eight widths.** Rejected for Stage 7. The
  existing `@i64-swap` covers the only frequent use case (sorting i64
  prefixes). New typed widths can spell swap as `get; get; set; set` until
  Stage 8 / Stage 9 wraps it.

## Related decisions

- [ADR 0038](0038-p2-writable-buffer.md) — `Buf` and the original
  anti-escape rule.
- [ADR 0061](0061-p3-stdlib-bundle-a-i64-vectors.md) — `I64Vec` and the
  shared scoped-lifetime model.
- [ADR 0073](0073-p4-function-values-and-closures.md) — closure capture
  policy and the `invalid-capture` diagnostic kind.
- [ADR 0084](0084-phase-6-fixed-width-integers.md) — fixed-width integer
  types and primitive naming convention.
- [phase-6-plan.md Q-P6-9](../plans/phase-6-plan.md) — closed by this ADR.
