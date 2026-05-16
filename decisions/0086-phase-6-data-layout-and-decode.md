# 0086 - Phase 6 data layout and decode support

**Status:** Accepted
**Date:** 2026-05-16
**Phase:** 6, Stage 8
**Closes:** [phase-6-plan.md Q-P6-10](../plans/phase-6-plan.md)
**Amends:** [ADR 0072](0072-p4-record-products.md),
[ADR 0084](0084-phase-6-fixed-width-integers.md), and
[ADR 0085](0085-phase-6-typed-mutable-memory.md) additively.

## Context

Phase 6 Stage 8 checks whether emulator-style state and instruction decode
need a new data-layout or tagged-union surface. The preceding Phase 6 stages
already provide the pieces most decode kernels need:

- structural records from Phase 4 for CPU/device state and decoded
  instruction records,
- fixed-width integers and bit operations from Stage 6 for opcodes, tags,
  flags, masks, and register values,
- typed vectors and byte-bus helpers from Stage 7 for RAM/register-file
  storage,
- existing `match` over integer patterns for small opcode dispatch tables.

The stage must not pull in Tacit-Full refinements, capabilities, handlers,
row polymorphism, arbitrary host FFI, untyped pointers, unchecked memory
access, or a full emulator.

## Decision

No new syntax or canonical node is added in Stage 8.

CPU and device state should be represented as structural records whose fields
are ordinary Tacit values: fixed-width integers for registers and counters,
`Bool` for flags, nested records for flag/status groups, and typed-vector
handles for local mutable storage when needed. Record field names remain
semantic labels, not ABI offsets.

Decoded instructions and addressing modes should be represented as explicit
records with fixed-width tag fields, for example `{kind: u8, mode: u8,
operand: u8}`. Opcode dispatch uses `match` over fixed-width or legacy
integer values, with `pat-int` arms for known opcodes and an explicit wildcard
arm for fallback/illegal decode. This is sufficient for Phase 6 decoder
skeletons and avoids introducing an enum declaration system before the
module/package and host-interface surfaces are complete.

Existing constructor nodes are not promoted into a typed ADT surface in
Stage 8. Constructors remain available in the canonical grammar, and `True`
/ `False` continue to typecheck as `Bool`, but instruction decode examples
must not depend on user-defined constructor types. A future ADT stage may
revisit this once exhaustiveness, constructor namespaces, module visibility,
and host ABI representation can be designed together.

ABI-stable record layout is deferred to Stage 10. Stage 8 records are
language-level structural products. The compiler may continue to lower them
using its internal sorted-field representation, and hosts must not depend on
that layout. Any host-exposed product shape in Stage 10 must opt into a
separate, explicit ABI metadata surface.

Packed layout is also deferred to Stage 10. Programs that need byte-level
packing during Phase 6 should use `u8vec` and the Stage 7 typed load/store
helpers. No packed-record syntax, alignment attribute, bitfield declaration,
or unchecked pointer reinterpretation is added.

Inspection rendering does not change. Existing inspection output for records,
projections, fixed-width type names, primitive symbols, and `match` arms is
the Stage 8 rendering surface.

No new structured diagnostics are added because Stage 8 adds no new accepted
forms. In particular:

- `non-abi-safe-layout` is deferred until Stage 10 introduces ABI-expressible
  metadata,
- `unsupported-packed-layout` is deferred until a packed layout syntax exists,
- static exhaustiveness diagnostics are deferred until Tacit has typed ADTs or
  a finite tag declaration surface.

Non-exhaustive executable `match` lowering remains the existing deterministic
runtime exit path. Stage 8 examples use wildcard arms when fallback behavior
is part of the decode contract.

## Consequences

- A CPU-state record and instruction decoder can be expressed with the
  existing Phase 4 plus Phase 6 surface.
- Phase 6 does not add a second product representation or a premature enum
  system.
- Host ABI layout remains explicit future work instead of being inferred from
  structural record order.
- Packed decode remains byte-buffer work, not pointer reinterpretation.
- Performance-sensitive lowering choices, including match-table lowering and
  record scalar replacement, remain Phase 8 work.

## Rejected alternatives

- **ABI-stable records by default.** Rejected. Tacit records are structural
  products with canonical sorted fields. Treating that order as a host ABI
  would freeze an implementation detail before Stage 10 defines ownership,
  allocation, and metadata.
- **Packed records and bitfields.** Rejected. Stage 7 `u8vec` byte-bus helpers
  already cover packed byte decoding safely. Packed fields would need
  alignment, endian, and host-boundary rules that belong with the ABI.
- **User-defined enum syntax for instructions.** Rejected for Stage 8. Decode
  records with explicit tags are enough for emulator skeletons, while real ADT
  syntax would require constructor namespaces, exhaustiveness, import/export
  behavior, and ABI representation decisions.
- **Static exhaustiveness for arbitrary integer matches.** Rejected. Without a
  declared finite tag space, the checker cannot distinguish intentionally
  partial integer dispatch from a bug without noisy false positives.

## Related decisions

- [ADR 0072](0072-p4-record-products.md) - structural record products.
- [ADR 0084](0084-phase-6-fixed-width-integers.md) - fixed-width integer and
  bit primitive surface.
- [ADR 0085](0085-phase-6-typed-mutable-memory.md) - typed mutable memory and
  byte-bus helpers.
- [phase-6-plan.md Q-P6-10](../plans/phase-6-plan.md) - closed by this ADR.
