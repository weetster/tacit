# 0094 - Tacit-owned package instances

**Status:** Accepted (design only; implementation deferred to a follow-up
stage commit)
**Date:** 2026-05-18
**Phase:** Stateful host-bridge, Stage 3
**Closes:** [stateful-host-bridge-plan.md Stage 3 design](../plans/stateful-host-bridge-plan.md),
Q-SHB-2, Q-SHB-3, Q-SHB-4, Q-SHB-5
**Amends:** [ADR 0080](0080-phase-6-module-semantics.md),
[ADR 0085](0085-phase-6-typed-mutable-memory.md),
[ADR 0088](0088-phase-6-host-interface-abi.md), and
[ADR 0092](0092-rich-boundary-library-codegen.md) additively.

## Context

Stages 1 and 2 of the stateful host-bridge track delivered rich boundary
codegen (ADR 0092) and a bounded-stack iteration primitive (ADR 0093). Bulk
buffers cross the host ABI, and tight emulator loops compile to native
back-edges. What is still missing is **persistent state across host calls**.

Today every public Tacit export is a pure function over its arguments.
Stack-allocated typed vectors (ADR 0085) die at the export return; mutable
records do not exist; closures cannot capture non-escapable handles. A
Tacboy-shaped host that wants to run one CPU step at a time would have to
push all of CPU registers, WRAM, VRAM, OAM, framebuffer, and audio queues
across the boundary on every call, or move the entire emulator state into the
host and reduce Tacit to a per-instruction helper. Neither matches the goal
declared in ADR 0091: Tacit owns long-running application state; the host
bridges to platform and third-party libraries.

Stage 3 adds the missing piece: **Tacit-owned package instances**. A
stateful package declares a state record once; the host creates an opaque
instance handle; Tacit methods operate on per-instance memory across many host
calls; the host destroys the instance when done.

This stage answers four flagged design questions:

- **Q-SHB-2** (canonical representation for state declarations) — resolved by
  a new `(state ...)` entry inside the unit's `defs` list.
- **Q-SHB-3** (heap vectors: new handle family or extension of typed vectors?)
  — resolved by reusing the ADR 0085 typed-vec handle types as the shape of
  state vec fields. Heap ownership lives inside the instance; the handle the
  user sees through `@state-load` is an ordinary borrowed handle with method-
  call lifetime, governed by the existing anti-escape rule.
- **Q-SHB-4** (allocation failure representation) — resolved by a new ABI
  status `TACIT_STATUS_OUT_OF_MEMORY`. Tacit source does not see allocation
  failure; the wrapper surfaces it to the host.
- **Q-SHB-5** (host callbacks receiving Tacit-owned borrowed slices) —
  resolved by extending the Stage 1 call-local borrow rule symmetrically: a
  Tacit method may pass a borrowed slice of its instance memory into a host
  callback for the dynamic extent of that callback.

No design, implementation, or validation work for this stage may read, list,
search, or otherwise depend on `corpus/sealed/`.

## Decision

### Stateful units

A *stateful unit* is a `unit` whose `defs` list contains exactly one
`(state ...)` entry. The presence of that entry switches the unit's compiled
host-interface artifact from the Stage 1 stateless shape to the Stage 3
instance shape. A unit without `(state ...)` continues to behave exactly as
ADR 0088 / ADR 0092 define it.

A unit may contain at most one `(state ...)` entry. Multiple state types per
unit (or per package) are explicitly out of scope for Stage 3.

### Canonical state declaration

One new canonical node kind, embedded as a `defs`-list entry:

| Tag | Arity | Children | Notes |
| --- | --- | --- | --- |
| `state` | 2 | name-sym, record-ty | Declares the per-instance state shape. |

`name-sym` is a display alias (display-only, not used for identity); the state
type's canonical identity is the BLAKE3 of the `(state ...)` node itself.
`record-ty` is an ordinary `(record-ty ...)` node restricted to the field
types listed below.

The `unit` arity is unchanged. A `(state ...)` entry sits alongside `def`
entries inside `defs`. Existing units parse and check unchanged.

Authoring view introduces a `state` keyword that lowers to the canonical
form:

```tacit
unit Tacboy {
  state Self = {
    ram         : u8vec,
    vram        : u8vec,
    oam         : u8vec,
    framebuffer : u8vec,
    rom         : u8vec,
    ext_ram     : u8vec,
    pc          : u16,
    sp          : u16,
    a           : u8,
    ime         : Bool,
    ...
  }

  export public load_rom    : u8vec -> Int / {Alloc, Mut};
  export public set_input   : u8 -> Int / {Mut};
  export public run_frame   : Int -> Int / {Mut};
  export public read_frame  : u8vec -> Int / {Mut};
}
```

### State field types

A state record field's type must be one of:

- a fixed-width integer (`i8`/`u8` through `i64`/`u64`) or `Bool`;
- the legacy `Int`;
- a typed vector handle (`u8vec`, `i32vec`, etc.) — interpreted as a
  heap-allocated, instance-owned slot;
- a nested ABI-expressible record whose fields recursively satisfy this list.

Function values, closures, legacy `Buf`/`I64Vec`, owned strings, and unbounded
nested vectors-of-vectors are rejected by typecheck with
`state-field-shape-invalid`.

Scalar and record fields are zero-initialised when the instance is created.
Vec fields start with `data = null, len = 0`; they must be allocated
explicitly through `@state-alloc-vec` before they can be read or written
through the vec primitives.

### Implicit self threading

In a stateful unit, every public export is rewritten by the typechecker to
take a hidden `self : Self` first parameter. The rewrite is invisible at the
source signature level (the export's declared signature does not mention
`self`), but it is the *only* hidden contract introduced; field access and
allocation through the primitives below are explicit and greppable.

Within the body of any public export of a stateful unit, and within `rec`
helpers and combinator callbacks reachable from such a body, the symbol
`@self` resolves to the hidden parameter and has type `Self`. The state-access
primitives below are valid only where `@self` is in scope.

Private `def` entries of a stateful unit may also reference `@self` and the
state primitives, but only when invoked from a public export of the same unit.
The check is the same shape as ADR 0085's non-escapable-handle check: a
private helper that depends on `@self` may not be exported (publicly or
package-visibly), may not be passed as a first-class function value, and may
not be captured by an escaping closure.

The hidden parameter does not change the canonical hash of an export's body
(the body still references `@self`, not `(var 0)`), so package definition
hashes remain stable across compilers that implement this rewrite differently.

### State-access primitives

Five new compiler-recognised `@`-primitives. They follow the
[ADR 0074](0074-p4-higher-order-combinators.md) pattern: no new canonical
node, recognition lives in the typecheck and codegen primitive tables.

| Primitive | Signature (informal) | Effect | Notes |
| --- | --- | --- | --- |
| `@state-load FIELD` | `() -> T` | `{}` | Read a scalar/record field, or borrow a vec field for the rest of the enclosing method call. |
| `@state-store FIELD VALUE` | `T -> Int` | `{Mut}` | Write a scalar/record field. Not valid for vec fields. |
| `@state-alloc-vec FIELD COUNT` | `Int -> Int` | `{Alloc, Mut}` | Allocate the named vec field with `COUNT` elements. Field must currently be length 0. Returns 0 on success; on OOM the enclosing export terminates with `TACIT_STATUS_OUT_OF_MEMORY`. |
| `@state-free-vec FIELD` | `() -> Int` | `{Mut}` | Free the named vec field. Sets it back to `data = null, len = 0`. Idempotent on already-empty fields. |
| `@state-slice FIELD OFF LEN` | `Int -> Int -> u8vec` | `{}` | Convenience for `(@u8vec-slice (@state-load FIELD) OFF LEN)`. Only valid for `u8vec` fields; symmetric versions for other widths follow the `@<ty>vec-slice` naming. |

`FIELD` is a literal field-symbol child of the primitive's canonical-text
form, like the field selector in existing record projection. It is resolved
against the unit's `(state ...)` declaration at typecheck time; an unknown
field is reported as `state-field-unknown`.

A vec field accessed through `@state-load` returns a borrowed typed-vec
handle whose lifetime is the *enclosing method call*. The handle is
non-escapable in exactly the ADR 0085 sense: it may be passed to vec
primitives, to direct-call `rec` helpers, to combinator callbacks, and as a
borrowed parameter to a host callback (see Q-SHB-5 below); it may not be
captured by an escaping closure, stored in another record, or returned. There
is no new "owned vec" handle type — ownership is implicit in the state field
itself, which is owned by the instance.

### Instance lifecycle ABI

A stateful unit's host interface adds three ABI functions in addition to the
ordinary per-export wrappers:

```c
tacit_status tacit_p_<pkg>_create(
    tacit_p_<pkg>_context *ctx,
    tacit_p_<pkg>_instance **out);

tacit_status tacit_p_<pkg>_destroy(
    tacit_p_<pkg>_context *ctx,
    tacit_p_<pkg>_instance *instance);

tacit_status tacit_p_<pkg>_e_<def>(
    tacit_p_<pkg>_context *ctx,
    tacit_p_<pkg>_instance *instance,
    /* flattened Tacit source-level parameters... */
    result_type *out);
```

`tacit_p_<pkg>_instance` is an opaque forward-declared struct. The host may
hold, pass, and compare pointers to it; the host may not dereference, copy,
or inspect its contents. The struct layout is an implementation detail.

`create` allocates the instance memory (the state record itself plus space
for any zero-length vec slots) using the host-process allocator. It does not
allocate any state vec contents — those are allocated by Tacit methods that
call `@state-alloc-vec`. On allocator failure, `create` returns
`TACIT_STATUS_OUT_OF_MEMORY` and leaves `*out` unchanged.

`destroy` walks every vec field, frees any non-zero-length contents through
`@state-free-vec` semantics, then frees the instance memory. Calling
`destroy` on a null pointer is a no-op that returns
`TACIT_STATUS_BAD_ARGUMENT`. Calling `destroy` twice on the same pointer is
undefined behaviour, in line with C ABI conventions; Stage 3 does not add
generation tracking.

Each public export of the stateful unit produces one wrapper symbol with the
hash-derived name `tacit_p_<pkg>_e_<def>` from ADR 0088, plus an instance
parameter inserted between `ctx` and the source-level parameters.

### Allocation failure

ADR 0088 status atoms gain one new value:

```c
typedef enum {
  TACIT_STATUS_OK              = 0,
  TACIT_STATUS_BAD_ARGUMENT    = 1,
  TACIT_STATUS_MISSING_IMPORT  = 2,
  TACIT_STATUS_HOST_ERROR      = 3,
  TACIT_STATUS_OUT_OF_MEMORY   = 4,
} tacit_status;
```

`OUT_OF_MEMORY` is produced by `create`, by per-export wrappers whose Tacit
body invokes a failing `@state-alloc-vec`, and by no other source.

The lowering of `@state-alloc-vec` failure to a returned status is an
implementation detail (setjmp/longjmp, threaded status, or thread-local
sentinel are all acceptable). The contract is:

1. On allocator success the primitive returns 0 and execution continues.
2. On allocator failure the current export terminates immediately with
   `TACIT_STATUS_OUT_OF_MEMORY`.
3. State that was already allocated before the failure is left intact on the
   instance — the host may inspect the instance via subsequent methods, or
   call `destroy` to release everything. No partial cleanup happens
   automatically inside the failing method.
4. The instance handle remains valid after `OUT_OF_MEMORY`; the host is
   responsible for calling `destroy`.

Tacit source has no syntactic form that catches `OUT_OF_MEMORY`. Allocation
failure is a wrapper-level termination, not a Tacit-level value.

### Host callbacks may receive instance-owned borrows (Q-SHB-5)

A Tacit method may pass a borrowed typed-vec handle into a host callback,
where the borrow's backing memory is owned by the current instance. The host
callback obligations are the same as in ADR 0092:

- the pointer is valid for the dynamic extent of the host callback's
  execution;
- the host callback must not store, return, copy out, or otherwise retain
  the pointer past its return;
- when the borrow has zero length, the pointer may be null.

The borrow rule is now symmetric:

| Direction | Origin of memory | Lifetime |
| --- | --- | --- |
| Host → Tacit (ADR 0092) | Host owns | Export call |
| Tacit → Host (this ADR) | Instance owns | Host callback call |

There is no Stage-3 mechanism for the host to retain a Tacit-owned pointer
across calls. A host that needs persistent access to instance memory must
copy it through a borrowed callback, or expose a polling method that the host
calls back into Tacit to read.

### Interface metadata extensions

`interface.json` (the `tacit-interface-v1` schema from ADR 0088) gains one
optional top-level key, emitted in the fixed order shown:

```json
{
  "format": "tacit-interface-v1",
  "abi": "tacit-host-abi-v1",
  "package_hash": "blake3:...",
  "instance": {
    "type_hash": "blake3:...",
    "create_symbol": "tacit_p_<pkg>_create",
    "destroy_symbol": "tacit_p_<pkg>_destroy",
    "state_fields": [
      { "name": "ram",  "kind": "vec", "element": "u8" },
      { "name": "pc",   "kind": "scalar", "name_type": "u16" },
      ...
    ]
  },
  "exports": [...],
  "imports": [...],
  "records": [...]
}
```

The `instance` block is present iff the package's unit declared
`(state ...)`. `state_fields` are listed in the canonical sorted record-field
order (ADR 0008). Field entries describe shape (`scalar`/`vec`/`record`) and
element width but do not expose layout offsets — those remain an
implementation detail of the Tacit codegen.

Each entry in the `exports` array of a stateful package additionally carries
`"instance_method": true`, signalling to binding generators that the C
signature includes the instance pointer.

### Generated C and Rust binding conventions

Generated headers emit the opaque struct, lifecycle functions, and per-method
wrappers as above. Generated Rust bindings expose:

```rust
pub struct Instance<'ctx> { /* private */ }

impl<'ctx> Instance<'ctx> {
    pub fn new(ctx: &'ctx Context) -> Result<Self, Error>;
    pub fn load_rom(&mut self, rom: &[u8]) -> Result<i64, Error>;
    pub fn run_frame(&mut self, ticks: i64) -> Result<i64, Error>;
    pub fn read_frame(&mut self, out: &mut [u8]) -> Result<i64, Error>;
    /* Drop impl calls destroy */
}
```

`Drop` is the canonical Rust binding for `destroy`. Manual `destroy` is not
exposed in the safe Rust binding; the underlying `unsafe extern "C"` symbol
remains available for hosts that want explicit control.

### Diagnostics

Stage 3 reserves these structured diagnostic kinds:

| Kind | Producer | Meaning |
| --- | --- | --- |
| `state-multiple` | parser/typecheck | A unit declares more than one `(state ...)` entry. |
| `state-field-shape-invalid` | typecheck | A state record field has a type outside the Stage 3 list (function, legacy buf, nested vec-of-vec, etc.). |
| `state-field-unknown` | typecheck | A state-access primitive named a field that does not appear in the unit's state declaration. |
| `state-field-wrong-kind` | typecheck | `@state-store` was used on a vec field, or `@state-alloc-vec` was used on a non-vec field, etc. |
| `state-access-outside-self` | typecheck | A state-access primitive appears in a location where `@self` is not in scope. |
| `state-helper-escapes-self` | typecheck | A private helper that depends on `@self` is exposed, returned, or captured by an escaping closure. |
| `state-alloc-not-empty` | runtime | `@state-alloc-vec` invoked on a field whose current length is non-zero. Surfaced as `TACIT_STATUS_BAD_ARGUMENT`. |
| `abi-instance-without-state` | interface | The interface generator was asked to emit an instance ABI for a unit with no `(state ...)` declaration. |

Per-export `abi-inexpressible-*` diagnostics from ADR 0088 continue to apply
to method signatures; the instance pointer is not part of the ABI
expressiveness check.

### Ownership and lifetime summary

- The host owns the instance pointer's identity and lifecycle (it decides
  when to call `destroy`).
- The instance owns its state record memory and every allocated vec field's
  backing memory.
- Tacit method calls borrow state vec contents for the duration of the call;
  borrows obey the existing non-escape rules.
- Host callbacks borrow Tacit-owned vec contents for the duration of the
  callback; the host must not retain the pointer.
- No Tacit-owned value is returned to the host as an owned value. Read-out is
  copy-out, performed by Tacit code through a host-allocated borrowed vec
  parameter.

## Alternatives considered

### Explicit `self : Self` parameter on every method

Rejected. Eliminates the only hidden contract in this design — at the cost of
adding a per-method signature line that repeats for every export, and at the
cost of introducing a new `ref-ty` / "opaque heap reference" type with its
own anti-escape rules. The implicit-self form keeps the explicit, greppable
surface where it matters (every read/write goes through `@state-load`,
`@state-store`, `@state-alloc-vec`) while saving primer surface and
signature-line repetition. The hidden parameter does not change canonical
body hashes (the body references `@self`, which is a stable named symbol).

### Heap-allocator primitives only, no instance concept

Rejected. Letting heap vec handles cross the ABI as record fields would
require relaxing ADR 0085's anti-escape rule with a "heap" handle variant
that has different lifetime semantics, plus a destructor story for handles
inside record fields, plus a public ABI in which the *entire* state record is
threaded by value through every method call. The opaque instance pointer
keeps the host signature stable as state evolves and keeps the existing
anti-escape rule exactly as it was.

### Tacit-level allocation failure (`{ ok : Bool, value : Self }`)

Rejected. Forces every allocation site to fan out into success and failure
branches, even though Tacit has no source-level recovery path that does
anything other than report. For Tacboy the only meaningful response to OOM is
"tell the host"; the ABI status `OUT_OF_MEMORY` does that in one place.

### Process abort on OOM

Rejected. The Tacboy slice loads cartridge-controlled buffer sizes (ROM,
external RAM); an attacker-controlled ROM that asks for a multi-gigabyte
external RAM should not crash the host process. Bounds violation traps
(ADR 0085) remain appropriate for logic bugs; allocation failure is a
resource condition that the host must be able to handle.

### Asymmetric borrow rule (host→Tacit only)

Rejected. Stage 1 (ADR 0092) already established host→Tacit call-local
borrows. The natural counterpart is Tacit→host call-local borrows, with the
same rule. Forcing a host-allocated buffer to be passed in just so the frame
can be copied into it adds a per-frame memcpy and a per-frame parameter
without buying any safety the symmetric rule does not already buy.

### Putting `(state ...)` in a new top-level slot on `unit`

Rejected. Changing `unit` arity (currently 3, per ADR 0080) would touch every
parser, emitter, hasher, and visitor in the codebase, and would require a
spec amendment to ADR 0080. Embedding `(state ...)` as a `defs` list entry
costs one new node kind and zero arity changes. The `(state ...)` entry
contributes to the unit's identity through its position in `defs` exactly
like a `def` entry does.

### Multiple state types per unit

Out of scope for Stage 3. The implicit-self threading rule assumes one state
type per unit. Multiple state types would force every state-access primitive
to carry a state-type discriminator, which expands the primitive surface and
the diagnostic surface for a use case (one package, multiple unrelated
instance types) that the plan does not motivate. A future ADR may revisit
this if a concrete need appears.

### Generation tracking on instance handles

Out of scope for Stage 3. Use-after-free of an instance pointer is undefined
behaviour, matching how C ABIs handle opaque handles. A future ADR may add a
generation counter if a higher-assurance host wants checked handles, but it
is not a Stage 3 deliverable.

## Consequences

- Tacit packages can own long-running mutable state across host calls
  through a single hidden parameter, an explicit state declaration, and a
  small primitive surface.
- The host ABI gains exactly three new symbol kinds per stateful package
  (`create`, `destroy`, and per-export wrappers with an instance parameter),
  and one new status atom.
- The Stage 1 borrow rule is now symmetric; host callbacks may receive
  borrowed slices of instance memory with the same call-local discipline.
- ADR 0085's anti-escape rule for typed-vec handles is preserved unchanged.
  Heap ownership is internalised to the instance; the handle the user holds
  is always a call-local borrow.
- The canonical text format gains one node kind (`state`) and one optional
  named symbol (`@self`). The `unit` arity is unchanged; existing units
  parse, hash, and check unchanged.
- Allocation failure is surfaced to the host as `TACIT_STATUS_OUT_OF_MEMORY`.
  Tacit source has no allocation-failure value type.
- The Tacboy vertical slice (Stage 5) has a designed target for instance
  state, frame transfer, audio push, ROM loading, and input polling.
- This ADR is design only. Implementation lands in a follow-up stage commit
  whose exit criteria are the Stage 3 exit criteria in
  [stateful-host-bridge-plan.md](../plans/stateful-host-bridge-plan.md).

## Related decisions

- [ADR 0008](0008-record-field-ordering.md) — sorted record field order,
  reused for state field ordering in metadata.
- [ADR 0035](0035-p2-effect-set-canonical.md) — fixed Lite effect lattice;
  state primitives use `{Mut}` and `{Alloc}` without expanding it.
- [ADR 0080](0080-phase-6-module-semantics.md) — unit/imports/exports/defs
  shape; `(state ...)` lives as a `defs` entry.
- [ADR 0085](0085-phase-6-typed-mutable-memory.md) — typed-vec handles and
  anti-escape rule, reused for state vec field borrows.
- [ADR 0088](0088-phase-6-host-interface-abi.md) — host-interface ABI;
  amended additively with the instance lifecycle and `OUT_OF_MEMORY` status.
- [ADR 0091](0091-stateful-host-bridge-scope.md) — track scope.
- [ADR 0092](0092-rich-boundary-library-codegen.md) — Stage 1 boundary
  codegen; borrow rule extended symmetrically here.
- [ADR 0093](0093-bounded-stack-loop-primitive.md) — Stage 2 bounded loops;
  `@state-*` primitives work inside `@loop` callbacks unchanged.
