# 0088 - Phase 6 constrained host-interface ABI

**Status:** Accepted
**Date:** 2026-05-16
**Phase:** 6, Stage 10 design
**Closes:** [phase-6-plan.md Q-P6-12](../plans/phase-6-plan.md),
[phase-6-plan.md Q-P6-13](../plans/phase-6-plan.md), and
[phase-6-plan.md Q-P6-14](../plans/phase-6-plan.md)
**Amends:** [ADR 0080](0080-phase-6-module-semantics.md),
[ADR 0082](0082-phase-6-package-manifest-lockfile-cache.md),
[ADR 0085](0085-phase-6-typed-mutable-memory.md),
[ADR 0086](0086-phase-6-data-layout-and-decode.md), and
[ADR 0087](0087-phase-6-source-level-stdlib-foundations.md) additively.

## Context

Phase 6 Stage 10 turns the host model from ADR 0022 into a concrete
embedding ABI. Tacit packages should compile to a linkable native artifact
that a C or Rust host can call, and Tacit code should be able to declare
host-provided operations with explicit type and effect signatures.

The stage is constrained by earlier Phase 6 decisions:

- Module and package identity is content-addressed by canonical units,
  definitions, imports, exports, and package hashes.
- Public package exports are hash-addressed definitions with explicit
  signatures.
- Source-level stdlib packages are ordinary packages, not privileged ABI
  escape hatches.
- Typed vector handles are non-escapable inside Tacit, and Stage 7 explicitly
  deferred host-provided buffers to the host-interface stage.
- Stage 8 deferred ABI-stable record layout and packed layout until this
  stage.
- General FFI remains out of scope: no arbitrary `extern "C"`, direct
  ecosystem-library bindings, dynamic plugin loading, or untyped pointer
  escape hatches.
- No design, implementation, or validation work may read, list, or otherwise
  depend on `corpus/sealed/`.

This ADR is the Stage 10 design artifact. Implementation follows in the
canonical parser/view changes, package interface generation, codegen wrappers,
C header generation, Rust binding generation, diagnostics, and host demo.

## Decision

### Host imports are canonical capability declarations

Stage 10 adds one canonical import entry kind:

| Tag | Arity | Children | Notes |
| --- | --- | --- | --- |
| `host-imp` | 3 | capability-str, operation-str, sig | Host-provided capability operation. |

`host-imp` entries live inside the existing `imports` table of a `unit`.
The `unit` arity does not change. An `imports` table may contain ordinary
Tacit-to-Tacit `imp` entries from ADR 0080 and `host-imp` entries from this
ADR.

A host import's identity hash is:

```text
host_import_hash = BLAKE3(canonical_text((host-imp CAPABILITY OPERATION SIG)))
```

`imports` entries are sorted by import identity hash:

- for `imp`, the identity hash is the imported definition hash child;
- for `host-imp`, the identity hash is the computed `host_import_hash`.

A Tacit body refers to a host import with the existing `(ref "<hash>")`
node, where the hash is the `host_import_hash`. The checker resolves that
reference from the containing unit's `host-imp` declaration, uses the declared
signature for type/effect checking, and does not require a Tacit definition
artifact to exist for that hash.

Host imports are function imports. A host-provided scalar or configuration
value must be represented as a function, usually one taking a unit-like empty
record. This avoids persistent host-owned values in Tacit's value graph.

Capability and operation strings are canonical ASCII labels, not external
linker symbols:

- `capability-str` is a dotted lower-case identifier such as
  `"tacit.host.log"` or `"demo.input"`;
- `operation-str` is a lower-case identifier with optional hyphens such as
  `"write-byte"` or `"poll"`;
- neither string may name a library path, dynamic symbol, header file, or
  platform object.

Generated host bindings satisfy `host-imp` declarations through a callback
table in a package-specific context object. Tacit source never names the C or
Rust function that will implement the operation.

Authoring syntax is additive and lowers to `host-imp`:

```tacit
unit Demo {
  import host log_byte : u8 -> Int / {IO}
    from capability "tacit.host.log" operation "write-byte";
}
```

The exact authoring layout is display syntax. The canonical `host-imp` entry
and its hash are the semantic facts.

### Interface generation is explicit and package-rooted

A host interface is generated for a checked package. Generation consumes:

1. the package's public exports,
2. all definition artifacts reachable from those exports through `ref`,
3. all reachable `host-imp` declarations across the package dependency
   closure,
4. advisory aliases from manifests and sidecars only for comments or wrapper
   names, never for identity.

The interface generator treats every public package export as a candidate host
export. If any public export is not ABI-expressible, interface generation
fails with a structured diagnostic. Ordinary package `check`, package tests,
and non-host compilation may still succeed; the rejection applies to the
host-interface generation step.

This conservative rule keeps `packages/<hash>/interface.json` deterministic
for a package hash. A future ADR may add a canonical way to mark only selected
public definitions as host exports. Stage 10 does not add a manifest-only
host-export selection table because manifest bytes are not part of package
identity.

Generated machine-readable interface metadata is written to:

```text
.tacit/cache/packages/<package-hash>/interface.json
```

Generated C headers, Rust bindings, object files, static libraries, and other
target-specific outputs live under `.tacit/derived/...`, not in the cache.

### Interface metadata schema

The metadata format is deterministic JSON:

```json
{
  "format": "tacit-interface-v1",
  "abi": "tacit-host-abi-v1",
  "package_hash": "blake3:...",
  "exports": [
    {
      "hash": "blake3:...",
      "symbol": "tacit_p_<package64>_e_<definition64>",
      "parameters": [],
      "result": { "kind": "scalar", "name": "i64" },
      "effects": []
    }
  ],
  "imports": [
    {
      "hash": "blake3:...",
      "declaring_package_hash": "blake3:...",
      "capability": "tacit.host.log",
      "operation": "write-byte",
      "callback": "tacit_p_<package64>_i_<import64>",
      "parameters": [{ "kind": "scalar", "name": "u8" }],
      "result": { "kind": "scalar", "name": "i64" },
      "effects": ["IO"]
    }
  ],
  "records": []
}
```

Rules:

- Top-level keys are emitted in the fixed order shown above.
- `exports` are sorted by exported definition hash bytes.
- `imports` are sorted by host import hash bytes.
- `records` are sorted by record type hash bytes.
- Effect lists use the ADR 0035 atom order.
- Hashes are rendered as `blake3:<64-hex>`.
- Advisory display aliases may appear only in optional metadata fields or
  generated comments; primary identity and primary symbols are hash-based.

The metadata is target-independent. It describes the logical C ABI surface.
Target-specific object format, static-library format, include paths, and Rust
crate layout are derived outputs.

### ABI-expressible types

The host boundary accepts a deliberately small monomorphic subset.

Scalar types:

| Tacit type | C ABI type | Notes |
| --- | --- | --- |
| `Bool` | `uint8_t` | `0` is false, `1` is true; other inbound values are `bad-argument`. |
| `Int` | `int64_t` | Legacy signed 64-bit scalar. |
| `i8` / `u8` | `int8_t` / `uint8_t` | Fixed-width Stage 6 scalar. |
| `i16` / `u16` | `int16_t` / `uint16_t` | Fixed-width Stage 6 scalar. |
| `i32` / `u32` | `int32_t` / `uint32_t` | Fixed-width Stage 6 scalar. |
| `i64` / `u64` | `int64_t` / `uint64_t` | Fixed-width Stage 6 scalar. |

The empty record type is the unit-like ABI type. Generated C headers define a
one-byte `tacit_unit` struct so arity remains explicit even though the value
has no source fields.

Record types are ABI-expressible when every field is ABI-expressible and no
field is a borrowed vector handle. Field order is the canonical sorted field
order from ADR 0008. Generated headers emit one named `struct` per distinct
record type in the interface metadata. The record type hash is
`BLAKE3(canonical_text(record-type-node))`, and the primary C struct name is
`tacit_r_<64-hex-record-type-hash>`. Field names are sanitized for C with the
original field names preserved in metadata. Packed layout, bitfields, custom
alignment attributes, and pointer reinterpretation are not supported.

Typed vector handles from ADR 0085 are ABI-expressible only as borrowed
function parameters:

| Tacit type | C ABI shape |
| --- | --- |
| `i8vec` / `u8vec` | `{ int8_t* data, uint64_t len }` / `{ uint8_t* data, uint64_t len }` |
| `i16vec` / `u16vec` | `{ int16_t* data, uint64_t len }` / `{ uint16_t* data, uint64_t len }` |
| `i32vec` / `u32vec` | `{ int32_t* data, uint64_t len }` / `{ uint32_t* data, uint64_t len }` |
| `i64vec` / `u64vec` | `{ int64_t* data, uint64_t len }` / `{ uint64_t* data, uint64_t len }` |

For a borrowed vector parameter:

- the host owns the backing memory;
- the pointer may be null only when `len == 0`;
- for `len > 0`, the pointer must be non-null and aligned for the element
  type;
- the borrow is valid only for the dynamic extent of the export call or host
  callback;
- Tacit must not store, return, capture, or nest the handle in a record;
- a host callback must not store any vector pointer it receives from Tacit.

The C type is mutable-capable. Source type/effect checking still controls
whether Tacit may write through it. Generated Rust bindings may expose shared
slice borrows for exports whose flattened effect set lacks `Mut`; otherwise
they must conservatively require mutable slice borrows for vector parameters.

These types are not ABI-expressible in Stage 10:

- function values as parameters, results, or record fields;
- captured closure values crossing the boundary;
- effect-polymorphic or type-polymorphic functions;
- function types containing `eff-var`;
- user-defined constructor or ADT values other than `Bool`;
- legacy `Buf` and `I64Vec` handles;
- typed vector handles as results or record fields;
- heap strings, owned arrays, opaque pointers, raw addresses, and nullable
  pointer values.

An exported definition may be implemented internally using closures, records,
or stack-allocated typed vectors. The rejection above applies to values that
cross the host boundary, not to ordinary Tacit implementation details.

### ABI-expressible functions

A public export is ABI-expressible when:

- the exported value has a monomorphic function type;
- flattening its curried `fn-ty` chain reaches a non-function result;
- every parameter and the final result are ABI-expressible;
- every call effect in the flattened chain is a concrete `eff-set`;
- the definition-evaluation effect is pure.

The generated ABI function performs a saturated call. A Tacit type such as:

```text
u8vec -> u16 -> {ok: Bool, value: u8} / {Mut}
```

becomes one C-callable export with two source parameters and one result
out-parameter. Partial application is not exposed at the host boundary.

The flattened export effect is the union of all concrete call effects in the
chain. The same flattening rule applies to host imports.

### C ABI

Phase 6 uses a native C ABI over LLVM-generated linkable artifacts.

Primary exported symbols are deterministic and hash-based:

```text
tacit_p_<64-hex-package-hash>_e_<64-hex-definition-hash>
```

Host callback fields use:

```text
tacit_p_<64-hex-package-hash>_i_<64-hex-host-import-hash>
```

Generated headers may add alias comments or inline wrappers when advisory
aliases are valid C identifiers and do not collide. The primary symbols above
are the stable ABI.

Every generated export has the logical C shape:

```c
tacit_status tacit_p_<pkg>_e_<def>(
    tacit_p_<pkg>_context *ctx,
    /* flattened Tacit parameters... */
    result_type *out);
```

If the Tacit result is unit-like, the `out` parameter is omitted. For every
non-unit result, `out` must be non-null.

Every generated host callback has the logical C shape:

```c
tacit_status (*tacit_p_<pkg>_i_<imp>)(
    void *user,
    /* flattened Tacit parameters... */
    result_type *out);
```

The package-specific context contains:

- a `void *user` pointer passed to host callbacks;
- a pointer to the generated callback table;
- no dynamic symbol lookup data;
- no allocator hooks in Stage 10.

All ABI calls use the target platform's `extern "C"` calling convention. They
are synchronous, non-varargs, and must not unwind across the boundary. Rust
bindings wrap the generated `unsafe extern "C"` declarations in ordinary Rust
functions where borrow and status rules can be checked.

### Result and error ABI

Tacit source errors remain ordinary return values. For example, a Tacit
`{ok: Bool, value: u8}` result is returned as an ABI record through the `out`
parameter. There is no exception or unwinding mechanism in Tacit-Lite.

The generated wrapper status reports boundary-level failures:

| Status | Meaning |
| --- | --- |
| `TACIT_STATUS_OK` | The Tacit call completed and wrote the result, if any. |
| `TACIT_STATUS_BAD_ARGUMENT` | The host passed an invalid context, null out pointer, invalid bool, or invalid borrowed vector. |
| `TACIT_STATUS_MISSING_IMPORT` | A required host callback was absent from the context. |
| `TACIT_STATUS_HOST_ERROR` | A host callback returned a non-OK status. |

A host callback that needs domain-level failure should normally encode it in
its Tacit return type. Returning a non-OK ABI status aborts the current Tacit
export call and returns that status to the host; Tacit source cannot catch or
recover from it.

Existing runtime traps, including Stage 7 bounds traps lowered through
`llvm.trap`, remain non-recoverable process aborts in Phase 6. They are not
converted into status values by this ADR.

### Ownership, lifetimes, and allocation

Stage 10 transfers no owned heap values across the host boundary.

- Scalars and records are copied by value.
- Borrowed vectors are host-owned call-local borrows.
- The host owns the package context and callback table.
- Tacit does not free host memory.
- The host does not free Tacit memory.
- No Tacit-owned string, vector, closure environment, or heap object is
  returned to the host.
- No host allocator is accepted by the generated context in Stage 10.

Source-level `Alloc` effects remain meaningful for Tacit allocation inside a
call, such as stack vector allocation. They do not imply that ownership crosses
the ABI.

If a future stage adds owned strings, owned arrays, long-lived host resources,
or callback values, it must define allocator hooks, destructor rules, and
lifetime ownership as a new ABI revision.

### Capability and effect declarations

Every `host-imp` declares:

- a capability label,
- an operation label,
- a concrete monomorphic type signature,
- concrete call effects in its `fn-ty` chain.

Host import effects use the existing Tacit-Lite effect atoms only. Stage 10
adds no user-defined effects, capability tokens, handlers, row polymorphism,
or new effect atom.

A host import's flattened effect set must include `IO`. It may also include
`Mut` or `Div` when the operation mutates borrowed Tacit-visible memory or may
not return. `Alloc` is valid only when the Tacit-visible semantics of the
operation allocate through existing Tacit allocation behavior; host-private
allocation is not exposed as a Tacit-owned allocation.

Capability labels are metadata for generated bindings and diagnostics. They
do not authorize arbitrary libraries. The host chooses how to implement the
callback, and Tacit only sees the declared operation type and effects.

### Compile targets

Phase 6 commits to LLVM-native linkable artifacts only:

- object files and static libraries are in scope;
- generated C headers are in scope;
- generated Rust bindings over the C ABI are in scope;
- WASM is not a Phase 6 target.

The interface metadata is intentionally not WASM-specific, so a later ADR may
reuse it for a WASM backend. Stage 10 implementation must reject a requested
WASM host target with `abi-unsupported-target`.

### Diagnostics

Stage 10 reserves these structured diagnostic kinds:

| Kind | Producer | Meaning |
| --- | --- | --- |
| `host-import-parse` | parser/view | A `host-imp` or authoring `import host` declaration is malformed. |
| `host-import-invalid-name` | parser/view | A capability or operation string is outside the allowed ASCII label grammar. |
| `duplicate-host-import` | typecheck/interface | Two host import declarations have the same `host_import_hash`. |
| `host-import-signature-mismatch` | typecheck/interface | A `ref` or metadata row disagrees with the declared host import signature. |
| `abi-inexpressible-export` | interface | A public export cannot be represented by the Stage 10 host ABI. |
| `abi-inexpressible-type` | interface | A parameter, result, or record field uses a type outside the ABI subset. |
| `abi-inexpressible-effect` | interface | A function crossing the boundary uses an effect variable or non-concrete call effect. |
| `abi-vector-position` | interface | A typed vector handle appears as a result, record field, or other non-parameter boundary value. |
| `abi-symbol-conflict` | interface | Advisory alias wrapper names collide; primary hash symbols remain valid. |
| `abi-unsupported-target` | cli/codegen | A requested host target is outside Stage 10, including WASM. |
| `host-import-unsatisfied` | header/binding tests | A generated host context lacks a required callback. |

Diagnostics should include display aliases when available and the stable
`blake3:<hash>` identity for the export, host import, package, or record type.

### Tests and examples

Stage 10 implementation tests must cover:

- deterministic parsing, emission, and hashing of `host-imp`;
- type/effect checking of host imports through ordinary `ref` resolution;
- deterministic `interface.json` generation;
- generated C header signatures for scalar, record, and borrowed-vector
  functions;
- generated Rust bindings that call at least one exported function;
- host callback satisfaction for at least one `host-imp`;
- rejection of function-value parameters, effect-polymorphic exports, legacy
  buffer handles, vector returns, and record fields containing vectors;
- rejection of WASM target selection in Phase 6;
- no test, fixture, or validation step reading, listing, or searching
  `corpus/sealed/`.

## Consequences

- Host-provided imports are content-addressed declarations, not string-based
  FFI bindings.
- `interface.json` is deterministic for a package hash because host-interface
  generation considers the package's public exports rather than a manifest-only
  selection table.
- C and Rust hosts get stable hash-based symbols and machine-readable
  metadata while display aliases remain advisory.
- Borrowed typed vectors can cross the host boundary in the narrow form Stage
  7 deferred: call-local host-owned parameters only.
- ABI-stable record layout exists only at the generated host boundary. Tacit's
  internal structural record layout remains an implementation detail.
- No allocator boundary is exposed in Phase 6 because no owned heap value
  crosses the ABI.
- WASM remains a candidate future backend, but Phase 6 implementation stays
  focused on native LLVM linkable artifacts.
- No Phase 6 work may use `corpus/sealed/` contents, paths, metadata, or
  feedback to validate this design.

## Rejected alternatives

### General `extern "C"` declarations

Rejected. Letting Tacit source name arbitrary C symbols would bypass
content-addressed identity, typed capability declarations, and ownership
rules. Host imports are callback slots generated from canonical
`host-imp` declarations instead.

### Manifest-only host export selection

Rejected for Stage 10. Manifest bytes do not participate in package identity,
but `interface.json` is stored under the package hash. A manifest-only
selection table would let two different interfaces claim the same package
cache path. Public exports are therefore the Stage 10 host-export set.

### Link host imports by generated external symbol names

Rejected. Direct link symbols would make host import satisfaction depend on
process-global names and platform linker behavior. A package-specific context
with a callback table is explicit, testable, and keeps operation labels out of
the dynamic linker namespace.

### Pass closures across the boundary

Rejected. Closure values carry code and environment representation choices
that are compiler-managed and not stable ABI data. Hosts call exported Tacit
functions through generated symbols and provide callbacks through the context
table; they do not receive or pass Tacit closure pairs.

### Allow effect-polymorphic host exports

Rejected. The C ABI has no representation for an effect variable, and Stage 10
does not add row polymorphism or runtime effect dictionaries. Host-facing
functions must have concrete monomorphic effects.

### Return owned buffers or strings

Rejected. Owned heap values require allocator selection, free functions,
destructors, lifetime transfer, and error cleanup rules. Borrowed vectors are
enough for Phase 6 embedding demos and keep allocator boundaries closed.

### Support WASM in Phase 6

Rejected. WASM fits the host model, but adding it now would force a second
backend and import/export lowering path before the native C/Rust ABI is proven.
The metadata format stays portable enough for a future WASM ADR.

## Related decisions

- [ADR 0022](0022-pure-kernel-host-model.md) - pure computational kernel and
  host-owned ecosystem integration.
- [ADR 0035](0035-p2-effect-set-canonical.md) - concrete effect-set atoms.
- [ADR 0073](0073-p4-function-values-and-closures.md) - closure
  representation and non-escapable capture rules.
- [ADR 0080](0080-phase-6-module-semantics.md) - units, imports, exports,
  signatures, and `ref` hashes.
- [ADR 0082](0082-phase-6-package-manifest-lockfile-cache.md) - package
  hashes, cache layout, and reserved `interface.json` path.
- [ADR 0084](0084-phase-6-fixed-width-integers.md) - fixed-width scalar
  types.
- [ADR 0085](0085-phase-6-typed-mutable-memory.md) - typed vector handles and
  host-provided buffer deferral.
- [ADR 0086](0086-phase-6-data-layout-and-decode.md) - record and packed
  layout deferral to Stage 10.
- [ADR 0087](0087-phase-6-source-level-stdlib-foundations.md) - host-backed
  stdlib wrapper conventions.
- [phase-6-plan.md Q-P6-12, Q-P6-13, Q-P6-14](../plans/phase-6-plan.md) -
  closed by this ADR.
