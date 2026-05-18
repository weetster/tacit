# 0095 - Host callback trait codegen

**Status:** Accepted (design only; implementation deferred to a follow-up
stage commit)
**Date:** 2026-05-18
**Phase:** Stateful host-bridge, Stage 4
**Closes:** [stateful-host-bridge-plan.md Stage 4 design](../plans/stateful-host-bridge-plan.md),
Q-SHB-6
**Amends:** [ADR 0088](0088-phase-6-host-interface-abi.md) and
[ADR 0091](0091-stateful-host-bridge-scope.md) additively.

## Context

Stages 1, 2, and 3 of the stateful host-bridge track delivered bulk-boundary
codegen (ADR 0092), a bounded-stack loop primitive (ADR 0093), and
Tacit-owned package instances (ADR 0094). A Tacboy-shaped package can now
own emulator state, run tight loops, and exchange borrowed buffers across the
host ABI.

The remaining bridge friction is on the **host side**. Today the interface
generator emits one `unsafe extern "C"` wrapper per host import, and the host
satisfies a flat callback table whose fields are advisory aliases of
hash-derived symbols:

```rust
ctx.callbacks.present_frame   = Some(present_frame_cb);
ctx.callbacks.poll_buttons    = Some(poll_buttons_cb);
ctx.callbacks.monotonic_nanos = Some(now_cb);
ctx.user = &mut host as *mut _ as *mut c_void;
```

For a Tacboy-shaped package with ~6–8 host imports this is mechanical but
error-prone: the host must produce correct `unsafe extern "C"` signatures
(pointer + length structs, status return), thread `*mut c_void user` to
recover `&mut Host`, and remember which callback slots exist. A single typo
or missing slot produces undefined behaviour or a silent
`TACIT_STATUS_MISSING_IMPORT` at the first call site, depending on which
mistake was made.

Stage 4 cleans up that surface by emitting one Rust trait per generated
package, grouping all of that package's host imports as safe-Rust trait
methods. The host implements the trait once and binds it via a single call:

```rust
impl TacboyCallbacks for MyHost {
    fn present_frame(&mut self, frame: &[u8]) -> Result<i64, Error> { ... }
    fn poll_buttons(&mut self) -> Result<u32, Error> { ... }
    fn monotonic_nanos(&mut self) -> Result<u64, Error> { ... }
}
ctx.bind_callbacks(host);
```

This stage answers one flagged design question:

- **Q-SHB-6** (which capability labels belong in a conventional profile versus
  project-local declarations) — resolved by **not introducing profiles**.
  Capability labels remain project-local declarations. A standardised
  catalog of cross-package capabilities is speculative until a second
  consumer of the same logical operation appears; revisiting profiles is a
  future ADR concern, not a Stage 4 deliverable.

No design, implementation, or validation work for this stage may read, list,
search, or otherwise depend on `corpus/sealed/`.

## Decision

### One Rust trait per generated package

The Rust binding generator emits one trait per generated package interface,
named after the package's display alias (advisory metadata from
`tacit.toml`), suffixed with `Callbacks`. The trait's methods correspond to
the package interface's `imports` array — i.e. every `host-imp` declaration
reachable from the package's public exports through the dependency closure,
exactly as ADR 0088 already defines for interface metadata.

If the interface's `imports` array is empty, no trait and no `bind_callbacks`
helper are emitted. Packages with no host imports keep their existing
binding shape unchanged.

### Trait naming

The trait name is `<DisplayAlias>Callbacks`, where `<DisplayAlias>` is the
package's `tacit.toml` `[package].name` field sanitised to a Rust PascalCase
identifier (`tacboy` → `Tacboy`, `tacit.host.demo` → `TacitHostDemo`).

If the alias is missing, empty, or sanitises to an invalid Rust identifier
(e.g. starts with a digit), the generator falls back to `PackageCallbacks`.
The fallback is deterministic and emits no diagnostic — the alias is
advisory metadata (ADR 0082); identity remains the package hash.

The trait lives in the generated Rust binding crate at the top level, where
hosts already import `Context`, `Error`, and the per-export wrapper
functions.

### Method naming

Each `host-imp` becomes one trait method. The method name is the operation
label sanitised to Rust snake_case (`present-frame` → `present_frame`,
`monotonic-nanos` → `monotonic_nanos`). The operation label grammar from
ADR 0088 (lower-case ASCII with optional hyphens) sanitises mechanically.

When two operations from different capabilities sanitise to the same method
name (e.g. `tacit.log::write` and `tacit.audio::write` would both produce
`write`), the generator disambiguates by prefixing the *last* capability
segment with an underscore (`log_write`, `audio_write`). If disambiguation
itself collides, the generator emits the structured diagnostic
`callbacks-method-collision` and refuses to emit the trait; the user
renames one operation to proceed.

### Method signatures

Each method signature is a direct projection of the `host-imp`'s flattened
type signature into safe Rust:

- The first parameter is always `&mut self`. Host-side mutation is a
  property of the Rust host, not of the Tacit-visible effect set; the
  `Mut` atom on the host import refers only to mutation visible to Tacit
  through borrowed slices.
- Scalars use the existing ABI scalar mapping from ADR 0088
  (`i8`/`u8`/.../`i64`/`u64`/`bool`).
- Records use the generated `tacit_r_<hash>` struct types already emitted
  by ADR 0088.
- Borrowed `<ty>vec` parameters become `&[T]` when the host import's
  flattened effect set lacks `Mut`, and `&mut [T]` when it includes `Mut`.
  This is the symmetric application of the existing ADR 0088 rule that
  governs Rust binding mutability for exports.
- The method return type is `Result<R, Error>` where `R` is the Rust
  projection of the Tacit return type (or `()` for unit-like results).
  `Ok(value)` corresponds to a `TACIT_STATUS_OK` ABI return with `*out =
  value`; `Err(Error::HostError(code))` corresponds to a non-OK ABI status
  surfaced to the calling Tacit code.

The `Error` enum is the existing generated host-side error type; no new
variants are introduced.

### `bind_callbacks` helper

The generator additionally emits one method on `Context`:

```rust
impl Context {
    pub fn bind_callbacks<H: <Pkg>Callbacks + 'static>(&mut self, host: H);
}
```

`bind_callbacks` boxes the host, stores it in the existing `user` field
(replacing the manual `*mut c_void` cast), and populates every callback
slot in the context with a monomorphised forwarder that:

1. Recovers `&mut H` from `user` through the boxed representation.
2. Unpacks ABI parameters into Rust values (slices from pointer+length,
   record structs by value).
3. Calls the trait method.
4. Marshals the `Result` into a status code and out-parameter write.

Each forwarder is a single generated function per host import, parameterised
over `H` and emitted in the binding crate. The forwarders are not part of
the C ABI; they are private Rust functions that satisfy the C ABI slots.

The existing per-symbol callback fields on `Context` remain public.
`bind_callbacks` is additive — hosts that want fine-grained control (e.g.
to mix trait-driven and manually-written callbacks) can still set individual
slots. A host that calls `bind_callbacks` and then overwrites a slot wins
on the slot it overwrote.

### Lifetime and ownership

`bind_callbacks` takes the host by value and stores it in `Context`'s
existing `user` slot via a boxed allocation. The host outlives the
`Context`. When `Context` is dropped, the boxed host is dropped. Hosts that
need shared ownership wrap their state in `Rc`/`Arc` before calling
`bind_callbacks`; the trait is implemented on the wrapper.

The `'static` bound on `H` exists because callback forwarders may be called
at any time during a Tacit export call, including from re-entrant host
callbacks invoked by Tacit. Stage 4 does not introduce scoped or borrowed
host bindings.

### Interface metadata

`interface.json` is unchanged. The trait is purely a Rust-side projection of
metadata that already exists in the `imports` array. C header generation is
unchanged; existing per-symbol `unsafe extern "C"` declarations remain the
canonical ABI surface.

A future ADR may add C++ binding generation or other host-language traits
using the same projection rule. Stage 4 only commits to the Rust target.

### Diagnostics

Stage 4 reserves these structured diagnostic kinds:

| Kind | Producer | Meaning |
| --- | --- | --- |
| `callbacks-method-collision` | binding generation | Two host imports in the same interface sanitise to the same Rust method name and capability-prefix disambiguation also collides. |
| `callbacks-bad-alias` | binding generation | The package's display alias is non-empty but sanitises to a Rust keyword (e.g. `type`, `move`). Caller should rename or accept the `PackageCallbacks` fallback by clearing the alias. |

The first is a real collision; the second is informational and the
generator falls back to `PackageCallbacks` if the user does not act.

### Non-goals

- **No profile packages, no stdlib capability catalog.** The earlier draft
  of this ADR proposed `kind = "host-profile"` packages under
  `stdlib/tacit/host/` shipping canonical declarations for log/time/video/
  audio/input/storage. That was rejected as speculative standardisation
  before any second consumer exists. Project-local `host-imp` declarations
  remain the only way to introduce capabilities.
- **No cross-consumer drift detection.** Without profiles there is no
  notion of "the same logical capability declared differently across
  packages." If a future second consumer of the same capability emerges and
  drift becomes a real problem, a follow-up ADR may reopen the profile
  concept.
- **No loop-safety enforcement.** Marking some host imports as "yielding"
  versus "loop-safe" was originally listed for Stage 4 but is moved to
  Stage 6 hardening as an optional lint (`yielding-in-loop`). The Lite
  effect lattice from ADR 0035 is unchanged.
- **No async, scheduling, or re-entrancy contract.** Callbacks remain
  synchronous and single-threaded. Re-entrancy from host into Tacit and
  back is permitted exactly as it is today.
- **No C++ or other host-language bindings.** Rust only.
- **No primer changes.** Host-side ergonomics are not a Tacit-Lite primer
  concern; ADR 0090's primer-bump gate is not tripped.

## Alternatives considered

### Profile packages (the earlier draft of this ADR)

Rejected. The full profile mechanism (a `kind = "host-profile"` package
shipping canonical `host-imp` declarations, per-profile generated traits,
profile-aware `interface.json`) pre-pays for a coordination problem (drift
across multiple consumers of the same capability) that does not exist
today. Tacboy is the only motivating consumer. Trait codegen alone delivers
the host-side ergonomics win without baking a stdlib capability catalog
before a second consumer validates the shape. Profiles can be layered on
top of trait codegen later if real drift pressure appears.

### Status quo (no change)

Rejected. The flat callback table forces every host to write `unsafe
extern "C"` wrappers and `*mut c_void` self-threading. Beyond the manual
labour, this surface is the most C-shaped friction in the toolchain and the
hardest for LLM-generated host code to get right: typos produce UB rather
than compile errors. Trait codegen converts that surface into a
compile-checked trait impl with named methods and `&mut self`, which is a
substantial reduction in foot-guns for the same ABI underneath.

### One trait per host import

Rejected. A package with six host imports would expose six unrelated traits
with no grouping, and hosts that share state across operations (almost
always, since `&mut self` is the same `Host` struct) would have to either
implement six traits on the same struct or use `Rc<RefCell<…>>` plumbing.
One trait per package groups the related methods and lets a single `impl`
block satisfy them all.

### Trait per dependency package

Rejected. Tacboy depends on stdlib packages; a stdlib package may declare
host imports (e.g. a future `tacit.io` extension). Per-dependency-package
traits would force the host to implement multiple traits — one for Tacboy's
own imports, one for each transitively imported package that contributes
host imports. Per-root-package collection keeps the host surface to one
trait per generated interface. If trait method collisions become common as
the closure grows, the `callbacks-method-collision` diagnostic forces
explicit disambiguation at that point.

### Generate forwarders without a trait (closure-based)

Rejected. A closure-based binding (`ctx.bind_present_frame(|host, frame|
…)`) cannot give compile-time errors for missing callbacks; the host
discovers omissions at runtime as `MISSING_IMPORT`. The trait shape lets the
Rust compiler enforce that every required method is implemented, which is
the single largest LLM-correctness benefit on the host side.

### Make `bind_callbacks` take `&mut H` instead of `H`

Rejected for Stage 4. Borrowed binding would require a lifetime on
`Context` itself, propagating into every host API call. The `'static`-owned
shape is simpler and matches how `user` is already used. Hosts that need
shared mutable access wrap in `Rc<RefCell<…>>` or `Arc<Mutex<…>>` before
binding.

## Consequences

- The host side of the embedding ABI gets a safe, compile-checked Rust
  trait surface in place of `unsafe extern "C"` callback wiring.
- Forgetting to implement a host import becomes a Rust compile error
  ("not all trait items implemented: `present_frame`") instead of a
  runtime `TACIT_STATUS_MISSING_IMPORT`.
- Hash-derived symbol churn no longer reaches host source code: trait
  method names are derived from operation labels, which are part of the
  declared `host-imp` and only change when the user changes the signature.
- The C header and `interface.json` are unchanged. The C ABI surface
  remains the canonical contract; the trait is a Rust-side projection.
- The stdlib `tacit.host` namespace remains reserved (per ADR 0087) but
  is not populated by Stage 4. Project-local capability declarations remain
  the only way to introduce host imports.
- The agent workflow asset
  (`share-assets/workflow/agent-workflow.md`) gains a short host-side
  example showing the trait impl pattern; no other workflow changes are
  required.
- The primer is unchanged, so ADR 0090's toolchain patch-bump gate is not
  tripped.
- This ADR is design only. Implementation lands in a follow-up stage
  commit whose exit criteria are the Stage 4 exit criteria in
  [stateful-host-bridge-plan.md](../plans/stateful-host-bridge-plan.md).

## Related decisions

- [ADR 0035](0035-p2-effect-set-canonical.md) — fixed Lite effect lattice;
  trait method mutability follows the existing `Mut` rule.
- [ADR 0082](0082-phase-6-package-manifest-lockfile-cache.md) — package
  manifest schema; `name` field used as advisory trait alias.
- [ADR 0087](0087-phase-6-source-level-stdlib-foundations.md) — `tacit.host`
  namespace reservation; remains unpopulated by Stage 4.
- [ADR 0088](0088-phase-6-host-interface-abi.md) — host-interface ABI;
  trait codegen consumes its existing `imports` metadata unchanged.
- [ADR 0090](0090-toolchain-release-contract.md) — toolchain release
  contract; primer-bump gate not tripped because Stage 4 does not edit the
  primer.
- [ADR 0091](0091-stateful-host-bridge-scope.md) — track scope.
- [ADR 0092](0092-rich-boundary-library-codegen.md) — bulk-boundary
  codegen; trait methods receiving borrowed slices reuse the same null/len
  rules.
- [ADR 0094](0094-stateful-host-bridge-package-instances.md) — package
  instances; trait codegen applies identically to stateful and stateless
  packages.
