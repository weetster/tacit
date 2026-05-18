# 0092 - Rich boundary library codegen

**Status:** Accepted
**Date:** 2026-05-18
**Phase:** Stateful host-bridge, Stage 1
**Closes:** [stateful-host-bridge-plan.md Stage 1](../plans/stateful-host-bridge-plan.md)
**Amends:** [ADR 0088](0088-phase-6-host-interface-abi.md),
[ADR 0089](0089-phase-6-frozen.md), and
[ADR 0091](0091-stateful-host-bridge-scope.md) additively.

## Context

ADR 0088 defined a host-interface ABI where scalars, records, and borrowed
typed-vector parameters are ABI-expressible. Phase 6 implemented metadata,
C header generation, and Rust binding generation for that surface, but the
linkable static-library backend deliberately accepted only scalar boundary
types. Records and borrowed vectors could be described in `interface.json`,
but `tacit interface --emit-library` rejected them before codegen.

The stateful host-bridge track needs bulk transfer across generated native
libraries. ROM buffers, framebuffers, and audio buffers cannot require one
scalar ABI call per byte, pixel, or sample. The Stage 1 goal is to make the
already-designed ABI subset usable in linkable libraries before adding
persistent package instances.

No design, implementation, or validation work for this stage may read, list,
search, or otherwise depend on `corpus/sealed/`.

## Decision

The static-library backend now supports the non-owned ABI shapes that ADR 0088
already accepted:

- scalar parameters and scalar results;
- ABI record parameters and ABI record results, recursively containing
  ABI-expressible scalar or record fields;
- borrowed typed-vector parameters for exports and host callbacks.

Borrowed vectors keep the ADR 0088 ownership rule. The host owns the backing
memory, and the borrow is valid only for the dynamic extent of the call. A
borrowed vector may be an export parameter or a host-callback parameter. It is
still not ABI-expressible as a result, as a record field, or as an owned value
crossing the boundary.

Library codegen preserves Tacit's existing internal representation:

- scalars normalize to `i64` inside Tacit codegen;
- records use internal structural record values, with ABI-width
  marshalling at the wrapper boundary;
- borrowed vectors become non-escapable `VecHandle` bindings inside Tacit,
  carrying pointer plus length;
- host callback trampolines marshal internal values into the generated C ABI
  shape, call the host callback through the context table, and marshal results
  back into Tacit values.

Export wrappers validate borrowed-vector parameters with the ADR 0088 null
rule: `data == null` is accepted only when `len == 0`; otherwise the wrapper
returns `TACIT_STATUS_BAD_ARGUMENT`. Runtime bounds traps inside Tacit remain
ordinary non-recoverable traps rather than ABI status values.

Vector returns, vector record fields, function values, legacy `Buf`/`I64Vec`,
and owned strings or arrays remain rejected by host-interface generation.

## Alternatives considered

### Flatten records into scalar parameters

Rejected. The generated C/Rust metadata already exposes records by value.
Flattening only in library codegen would make the linkable symbol ABI diverge
from `interface.json` and generated headers.

### Treat borrowed vectors as raw pointer plus length parameters

Rejected at the public ABI. The C/Rust headers define named `tacit_<ty>vec`
structs, which keep the shape consistent and leave room for generated binding
ergonomics. The private Tacit trampoline may still flatten the handle
internally.

### Add owned vector returns now

Rejected. Owned values require allocator hooks, destructor rules, and
lifetime-transfer semantics. Stage 1 only completes the non-owned ABI subset
already designed by ADR 0088.

## Consequences

- `tacit interface . --emit-library` can now produce static libraries for
  packages whose public exports use ABI records or borrowed typed-vector
  parameters.
- Host callbacks can receive borrowed typed-vector parameters from Tacit
  calls, enabling bulk frame/audio/ROM-style transfer without scalar polling.
- Persistent Tacit-owned state is still future Stage 3 work. Borrowed vectors
  are call-local and host-owned.
- The old Phase 6 deferral that static-library codegen is scalar-only is
  closed for records and borrowed-vector parameters, but not for owned values.
