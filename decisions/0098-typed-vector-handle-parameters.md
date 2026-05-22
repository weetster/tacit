# 0098 - Typed vector handles as down-only call-local parameters

**Status:** Accepted
**Date:** 2026-05-22
**Phase:** Phase 6 corrective amendment (bounded codegen slice)
**Amends:** [ADR 0073](0073-p4-function-values-and-closures.md) and
[ADR 0085](0085-phase-6-typed-mutable-memory.md) additively. Bounded
reopening of Phase 6 codegen under the allowance in
[ADR 0089](0089-phase-6-frozen.md). Relates to
[ADR 0088](0088-phase-6-host-interface-abi.md).

## Context

Factoring systems-style Tacit code into reusable helpers — a memory bus, a
register file, an instruction decoder — naturally produces package-level
`def` artifacts whose parameters are typed vector handles, for example a
helper with the explicit signature `u8vec -> Int -> Int`. The type checker
already accepts such signatures: `Ty::Vec(FixedIntTy)` and `Ty::I64Vec` are
ordinary parameter types, and `infer.rs` imposes no restriction on a handle
appearing in parameter position.

Codegen does not implement them. Compiling a host library
(`tacit interface . --emit-library`) over a package whose helpers take
handle parameters fails with:

```
codegen does not yet support typed vector handle used in integer-value position
```

The cause is structural. A typed vector handle is not a first-class
`ValueTy`; it is a separate `Binding::VecHandle { ptr, len, ty }` kind. The
whole call ABI — signature construction, function hoisting, and the call
itself — is expressed over `Vec<ValueTy>`, so a handle cannot be named as a
declared parameter. Handles successfully flow in only three places today:

1. as local `@<ty>vec-alloc` bindings consumed by vec primitives in the
   *same* function body;
2. as hidden two-word `(ptr, len)` captures into direct-call `rec` helpers
   (ADR 0085, ADR 0059);
3. as a top-level library-export parameter, decoded by the export wrapper
   from the `BorrowedVector` host ABI.

Because `package_library` expands every internal definition reference inline
before codegen, an intra-package helper call is not a cross-function call at
all: it is `App(Lam, handle_arg, ...)`. Lowering routes every argument
through the value-expression path, and a handle argument there hits the
error above. ADR 0085 anticipated handles reaching `rec` helpers as hidden
captures, but it did not provide for a handle as an *explicit declared
parameter*, which is the natural authoring form for a factored helper.

Every Phase 6 typed-memory and data-layout example sidesteps this by keeping
all handle operations in one flat function body. The gap is genuine,
untested codegen — not authoring misuse — and resolving it ratifies a
memory-borrowing contract that the existing surface only ever specified as a
patchwork of unrelated predicates.

This amendment must not:

- make handles returnable, storable in records, or capturable by escaping
  closures;
- introduce lifetimes, regions, row polymorphism, capabilities, or a borrow
  checker;
- change the fixed Tacit-Lite effect lattice;
- change the host-interface `BorrowedVector` ABI or the C header / Rust
  bindings layout (ADR 0088);
- give first-class closure *values* handle-typed parameters.

## Decision

### The call-local borrow contract

A typed vector handle (`Buf`, `I64Vec`, and the eight `<ty>vec` types) is a
**call-local borrow**. It may travel *down* the call tree and nowhere else:

- **Down (permitted):** a handle may be passed as an explicit argument to a
  direct-call function. The callee may use it, and may forward it as an
  argument to further direct-call functions. The callee always returns
  before the handle's allocating `let` scope ends, because the caller's
  frame outlives the callee's.
- **Up (forbidden):** a handle may not be returned from a function.
- **Outward (forbidden):** a handle may not be stored in a record field,
  captured by an escaping first-class closure, used as `@loop` state, or
  exported as a library result.

This is one rule. Every pre-existing handle restriction — the
`invalid-capture` / `escape-vec-handle` diagnostics (ADR 0073, ADR 0085),
the record-field rejection and result-position rejection at the host
boundary (ADR 0088), and the loop-state restriction (ADR 0093) — is an
instance of it. The rule is stated, not changed: this ADR names the
contract and adds the one missing *down* edge (explicit parameters), it
does not widen what may escape.

The contract is **locally checkable with no lifetime variables.** A handle
parameter is valid for exactly the callee body. Reasoning about a handle
never requires knowing where it was allocated or how long it lives — the
mental model is one sentence: *a handle parameter is live for this body;
you cannot return it or store it.* This locality is a deliberate
language property, chosen so that an author — human or model — never has to
perform non-local lifetime reasoning.

### Handle parameters

A `def` or lambda may declare a parameter whose type is a handle type. Such
a function is a **direct-call function**: it is reachable through the
inlined `App(Lam, ...)` form that `package_library` produces, through `rec`
helper direct calls, and as a library export wrapper. It may forward a
handle parameter as an argument to another direct-call function.

A function that declares a handle-typed parameter may not be reified into a
first-class closure *value*. Handle-typed parameters on first-class closure
values are out of scope for this amendment and deferred; the inlined
direct-call form covers the package-helper use case that motivates it.

### One uniform internal handle representation

A handle argument lowers to a two-word `(ptr, len)` pair: an LLVM pointer
followed by an `i64` length. This is exactly the representation already used
for `rec` hidden captures. There is one internal handle calling convention,
used uniformly for handle parameters, handle captures, and handle arguments;
an author sees a handle passed like any other argument.

The host-interface boundary is unchanged. The `BorrowedVector` ABI and its
generated `tacit_<ty>vec` C struct (ADR 0088) keep their current layout. The
export wrapper continues to decode that struct into the internal two-word
form, so the struct is a thin boundary adapter over the same representation.
**Generated C headers and Rust bindings are byte-identical before and after
this amendment**, and host embedders need no rebuild.

### Compatibility

The change is strictly additive: it turns a codegen error into a successful
compilation. No program that compiles today stops compiling — the down /
up / outward restrictions are pre-existing and enforced by the type checker,
and none is added here. Definition hashes, lockfiles, and exact-hash imports
are computed over the AST and are unaffected by a codegen lowering change.
Tacit has no stable Tacit-to-Tacit binary ABI; every consumer recompiles
from content-addressed source, so no stale artifact can mismatch the new
lowering.

This is nonetheless a compiler behavior change and requires a new released
toolchain version. A project pinned to an earlier toolchain
(`tacit-toolchain-pin-v1`) keeps the prior behavior — handle parameters
still fail to compile — until it bumps its pin. Bumping only ever adds
capability.

## Diagnostics

No new diagnostic kind is introduced. The existing `escape-vec-handle` /
`invalid-capture` diagnostics already reject the *up* and *outward* cases at
type-check time. The codegen errors `typed vector handle used in
integer-value position` and `buffer-like handle used in integer-value
position` cease to fire for the *down* case (handle as a direct-call
argument); they remain for any genuine attempt to use a handle where an
integer value is required.

## Alternatives considered

### Do nothing — document handle parameters as unsupported

Rejected. It forces systems-style packages to keep all handle work in a
single monolithic function body, which defeats ordinary factoring of a bus
or decoder into helpers. The type checker already accepts the signatures, so
the status quo is an unannounced codegen / spec mismatch rather than a
designed limitation.

### Fully first-class handles (returnable, storable)

Rejected for this amendment. Making handles returnable or storable in
records requires lifetime or region tracking to remain sound, which
reintroduces non-local "is this handle still valid here?" reasoning — the
borrow-checker reasoning burden the call-local model exists to avoid. The
call-local contract is a strict subset of fully-first-class handles, so a
future ADR can extend additively if real pressure appears; choosing the
minimal contract now forecloses nothing.

### Two lowering regimes (inline beta-reduction internally)

Rejected. Because `package_library` inlines internal definitions, handle
arguments could be beta-reduced at `App(Lam, handle)` sites internally while
the `BorrowedVector` ABI is kept only at the host boundary. That is the
smallest code change, but it creates an invisible rule — a handle parameter
compiles only because the call happened to be inlined — and a single
uniform representation is the canonical-way choice and the one an author
expects.

### Handle parameters on first-class closure values

Deferred. A first-class closure value that takes a handle parameter is sound
only with a per-call-site anti-escape check on the handle argument, and it
is not needed for the package-helper use case. Keeping this amendment to
direct-call functions holds the reopened slice bounded.

## Consequences

- Systems-style packages may factor handle logic into ordinary helpers with
  explicit handle-typed parameters; `tacit interface . --emit-library`
  compiles them.
- The memory-borrowing contract for handles is stated once, as a single
  call-local rule, instead of living implicitly across several diagnostics.
- There is one internal handle calling convention, shared by parameters,
  captures, and arguments.
- The host-interface ABI, generated headers, and Rust bindings are
  unchanged; host embedders are unaffected.
- The fixed Tacit-Lite effect lattice is unchanged; no new diagnostic kind
  is added.
- Implementation is a follow-up task: codegen must thread a handle
  parameter kind through signature construction, function hoisting, and the
  call path, and lower a handle argument as the two-word `(ptr, len)` pair.
  A regression example under `examples/phase-6/` should factor a memory bus
  into handle-taking helpers so the previously failing path is exercised.
