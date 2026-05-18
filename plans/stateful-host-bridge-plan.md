# Stateful Host-Bridge Plan

**Status:** Draft; Stage 0 complete by
[ADR 0091](../decisions/0091-stateful-host-bridge-scope.md)
**Scope:** Future Tacit language and toolchain work to let Tacit own
long-running application state while the host only bridges to third-party
libraries and platform services.

## Context

Phase 6 proved that Tacit packages can be checked, tested, compiled to a
native static library, and called from a Rust host through a constrained ABI.
It also added fixed-width integers, typed mutable vectors, records, package
tests, host imports, generated C headers, and generated Rust bindings.

That is enough for small scalar kernels and emulator-style examples, but it is
not enough for a full emulator architecture where Tacit owns CPU state, RAM,
VRAM, OAM, timers, PPU state, audio queues, and execution control across many
host calls. The current constraints are:

- linkable library codegen accepts scalar boundary types only, even though
  interface metadata can describe records and borrowed vectors;
- typed vectors allocated inside Tacit are stack-scoped and die at export
  return;
- there is no Tacit-owned package instance or persistent mutable state across
  host calls;
- long-running loops are encoded through recursion today, without a dedicated
  loop-lowering contract suitable for millions of emulator steps;
- host imports are callback capabilities, not arbitrary FFI, which is the
  right model but needs richer bulk-data shapes for video, audio, input, and
  ROM transfer.

This plan treats a Game Boy emulator such as Tacboy as a representative stress
test. The target architecture is:

```text
Rust/C host:
  - owns SDL, audio device, window, OS files, timers, and controller plumbing
  - creates a Tacit emulator instance
  - forwards host callbacks to third-party libraries

Tacit package:
  - owns emulator state and memory
  - executes CPU/PPU/APU/timer/interrupt semantics
  - calls typed host capabilities for frame/audio/input/file bridge operations
```

No stage may use `corpus/sealed/` contents, paths, metadata, or feedback.

## Goal

Enable Tacit to own long-running stateful application logic while preserving
the constrained host model: Tacit declares typed capabilities; the host
satisfies those capabilities and bridges them to platform libraries. The host
should not need to implement emulator semantics or manually hold emulator
registers, memory maps, or device state.

## Non-Goals

- No arbitrary `extern "C"` declarations in Tacit source.
- No direct Tacit bindings to SDL, OpenGL, Vulkan, ALSA, CoreAudio, filesystem
  APIs, or other ecosystem libraries.
- No untyped pointer escape hatches.
- No unchecked memory access by default.
- No global mutable variables without explicit package-instance ownership.
- No concurrency model, green threads, actors, or async runtime in this plan.
- No WASM backend commitment unless a later ADR explicitly scopes it.
- No semantic-version package solver or public registry work.

## Success Criteria

- A Tacit package can allocate and retain emulator-sized state across host
  calls without leaking or exposing raw pointers.
- Host exports and callbacks can pass records and borrowed typed vectors
  through generated linkable artifacts, not only through `interface.json`.
- Long-running loops compile to bounded-stack machine code without relying on
  optimizer luck.
- A host can drive a Tacit-owned emulator through a small API such as
  `create`, `load_rom`, `set_input`, `run_until_frame`, `read_frame`, and
  `destroy`.
- Bulk framebuffer/audio transfer avoids one scalar host call per pixel or
  sample.
- The host remains a bridge: platform resources and third-party library calls
  are outside Tacit; domain state and emulator semantics are inside Tacit.

## Stages

### Stage 0: Scope ADR And Benchmark Shape

**Status:** Complete 2026-05-18. Deliverables:
[ADR 0091](../decisions/0091-stateful-host-bridge-scope.md) and
[stateful-host-bridge-benchmark.md](stateful-host-bridge-benchmark.md)

**Purpose:** Lock the future phase boundary before implementation.

Work items:

- Write a scope ADR that names this as stateful host-bridge work, not general
  FFI.
- Define Tacboy as an open exemplar, not a frozen benchmark and not a
  requirement to read or depend on sealed corpus data.
- Define an emulator-shaped benchmark with CPU-step throughput, framebuffer
  transfer throughput, audio-buffer transfer throughput, and host-call
  overhead.
- Decide which results are gates and which are descriptive.

Exit criteria:

- A scope ADR is accepted. Done:
  [ADR 0091](../decisions/0091-stateful-host-bridge-scope.md).
- The benchmark shape is documented under `plans/`. Done:
  [stateful-host-bridge-benchmark.md](stateful-host-bridge-benchmark.md).
- The non-goals above are explicitly preserved. Done.

### Stage 1: Complete Rich Boundary Codegen

**Status:** Complete 2026-05-18. Deliverable:
[ADR 0092](../decisions/0092-rich-boundary-library-codegen.md)

**Purpose:** Make the already-designed ABI shapes usable in linkable
libraries.

Work items:

- Extend library codegen beyond scalar parameters and scalar results.
- Support ABI records in generated wrappers, generated C headers, generated
  Rust bindings, and wrapper status checks.
- Support borrowed typed-vector parameters for exports and host callbacks.
- Preserve the Stage 10 ownership rule: borrowed vectors are host-owned and
  call-local.
- Add tests for record parameters/results, borrowed `u8vec` parameters,
  borrowed mutable vectors under `{Mut}`, and rejection of vector returns or
  vector fields.

Exit criteria:

- `tacit interface . --emit-library` accepts ABI records and borrowed vector
  parameters when the metadata ABI accepts them. Done by
  [ADR 0092](../decisions/0092-rich-boundary-library-codegen.md).
- A host can pass a ROM buffer or framebuffer buffer to Tacit as a borrowed
  `u8vec`. Done for borrowed-vector export parameters.
- A Tacit callback can pass a borrowed audio/frame slice to the host within
  one call. Done for borrowed-vector host callback parameters.

### Stage 2: Explicit Bounded-Stack Loops

**Status:** Complete 2026-05-18. Deliverable:
[ADR 0093](../decisions/0093-bounded-stack-loop-primitive.md)

**Purpose:** Give long-running kernels a reliable execution primitive.

Work items:

- Design an explicit loop surface or a mechanically guaranteed self-tail-call
  lowering rule. Done: standalone `@loop` primitive (ADR 0093). `rec` keeps
  current semantics.
- Prefer a minimal canonical expansion if it avoids adding a new AST node; add
  a new node only if the ADR shows the existing `rec` surface is not enough.
  Done: no canonical-text-format change; recognition lives in the typecheck +
  codegen primitive tables, following the [ADR 0074](../decisions/0074-p4-higher-order-combinators.md)
  combinator pattern.
- Ensure loop-carried scalar and record state can be updated without growing
  stack. Done: state PHI on `Int` / `FixedInt` / record types; non-escapable
  handles rejected by the `loop-state-shape-invalid` diagnostic.
- Define effect behavior for loops: effects are the union of body effects,
  and possible nontermination remains represented by existing `Div` policy or
  a narrowly amended policy. Done: `@loop` adds `Div` to the union of init
  and step-callback effects.
- Add inspection rendering that makes loop-carried variables and effects clear.
  The form is `(app (app (sym loop) init) step)` so existing inspection
  handles it; no special renderer added.

Exit criteria:

- Tight loops over millions of iterations compile to bounded-stack native code.
  Done: `loop_counts_to_one_million_without_stack_overflow` execution test
  passes.
- CPU-step and scanline-style loops can be written without depending on LLVM
  optimization passes. Done: codegen emits a labeled basic-block loop with a
  PHI on state and a `br` back-edge — no optimizer pass involved.
- Existing recursive programs keep their semantics. Done: `rec` lowering is
  unchanged; ADR 0093 is purely additive.

### Stage 3: Tacit-Owned Package Instances

**Status:** Complete 2026-05-18 via
[ADR 0094](../decisions/0094-stateful-host-bridge-package-instances.md)
and the Stage 3 implementation. Closes Q-SHB-2, Q-SHB-3, Q-SHB-4,
Q-SHB-5.

**Purpose:** Add persistent state without arbitrary mutable globals.

Work items:

- Design a package-instance lifecycle in the host ABI: create, call methods,
  and destroy. Done by ADR 0094: opaque `tacit_p_<pkg>_instance*` plus
  `create`/`destroy` symbols plus per-export wrappers that take the instance
  pointer between `ctx` and the source-level parameters.
- Define an opaque instance handle at the generated C/Rust boundary. The host
  may hold and pass the handle but may not inspect Tacit-owned memory. Done
  by ADR 0094: forward-declared opaque struct; Rust binding uses `Drop` for
  destroy and never exposes the inner layout.
- Add Tacit-owned heap allocation for instance fields, including fixed-width
  vectors sized for RAM, VRAM, OAM, cartridge ROM/RAM, audio queues, and
  framebuffers. Done by ADR 0094: a new `(state RECORD-TY)` canonical entry
  plus `@state-alloc-vec` / `@state-free-vec` primitives. Vec fields start at
  zero length; the user allocates explicitly so that cartridge-controlled
  sizes (ROM, external RAM) can be set per call.
- Define destruction, failure cleanup, and ownership rules. If allocation can
  fail, define whether failure is represented as ABI status or a Tacit-level
  result. Done by ADR 0094: failure is the new `TACIT_STATUS_OUT_OF_MEMORY`
  ABI status; partial allocations remain on the instance after a failed
  method and are released by `destroy`.
- Keep package identity content-addressed. Instance creation must not depend
  on manifest-only semantic choices. Done by ADR 0094: `(state ...)` is a
  canonical-text node and contributes to the unit hash through its `defs`
  position; manifests carry no instance-shaping information.
- Decide whether state declarations live in canonical source, generated ABI
  metadata, or both. Done by ADR 0094: canonical source is authoritative; the
  generator emits a derived `instance` block in `interface.json` that
  describes the shape without exposing layout offsets.

Exit criteria:

- A Tacit package can retain mutable vectors and records across calls. Done:
  `(state ...)` declarations generate an opaque instance allocation, state
  field storage, and `@state-load` / `@state-store` /
  `@state-alloc-vec` / `@state-free-vec` / `@state-slice` lowering.
- The host can create and destroy multiple independent instances of the same
  package. Done: generated C/Rust bindings expose create/destroy symbols and
  instance-method wrappers with an opaque instance pointer.
- No raw Tacit pointer or allocator detail crosses the boundary. Done:
  `interface.json` emits only shape metadata and generated headers
  forward-declare the opaque instance type.

### Stage 4: Host Callback Trait Codegen

**Status:** Complete 2026-05-18. Design:
[ADR 0095](../decisions/0095-host-callback-trait-codegen.md). Implementation
landed in this Stage 4 commit. Closes Q-SHB-6.

**Purpose (revised):** Reduce host-side friction for Rust hosts that satisfy
package-level host imports. The original framing ("standardize the bridge
shape without baking third-party libraries into Tacit") proposed a
conventional capability profile catalog; ADR 0095 narrows the stage to
trait-shaped Rust binding ergonomics only and defers any stdlib catalog
until a real second consumer appears.

Work items:

- Define conventional capability labels and signatures for video, audio,
  input, monotonic time, logging, and storage. **Declined by
  [ADR 0095](../decisions/0095-host-callback-trait-codegen.md):**
  pre-shipping a stdlib capability catalog before a second consumer is
  speculative. Capability labels remain project-local declarations under
  ADR 0088.
- Keep capabilities as typed host imports; do not add direct bindings to SDL
  or any other library. **Preserved by
  [ADR 0095](../decisions/0095-host-callback-trait-codegen.md):** status
  quo from ADR 0088 retained.
- Add source-level stdlib helpers for common bridge patterns, such as frame
  presentation, audio-buffer push, input-state polling, and ROM loading into
  Tacit-owned memory. **Declined by
  [ADR 0095](../decisions/0095-host-callback-trait-codegen.md):** helper
  packages move to project-local code (initially under Tacboy in Stage 5)
  and may graduate to `stdlib/tacit/host/` only when a second consumer
  needs the same helper.
- Specify which callbacks may be called from long-running loops and what
  effects they carry. **Deferred to Stage 6 hardening:** loop-safety
  classification becomes an optional `yielding-in-loop` lint rather than a
  Stage 4 deliverable. The Lite effect lattice from ADR 0035 is unchanged.
- Add generated binding ergonomics so Rust hosts can satisfy these capability
  tables without manually copying hash-derived symbols. **Done by
  [ADR 0095](../decisions/0095-host-callback-trait-codegen.md):** per-package
  `<Pkg>Callbacks` trait emission plus a `Context::bind_callbacks` helper.
  Methods are named from operation labels; hash-derived symbols stay
  internal to the binding crate. Implementation lives in
  `crates/tacit-typecheck/src/interface.rs::emit_rust_bindings` and emits the
  trait, per-import monomorphised forwarders, a `BoundCallbacks` sentinel
  record (`#[repr(C)]`, first field is the C-ABI callbacks table), an
  `unbind_callbacks` helper, and a `Drop` impl on the context struct that
  reclaims both boxed allocations.

Exit criteria:

- A host can implement a small capability table and avoid emulator-domain
  state. **Already met by ADR 0088** (status quo); ADR 0095 improves the
  ergonomics of doing so.
- Tacit packages can use conventional capability imports across projects.
  **Declined by [ADR 0095](../decisions/0095-host-callback-trait-codegen.md):**
  no conventional cross-project catalog is introduced. Capability labels
  remain project-local. If real cross-consumer pressure appears later, a
  future ADR may reopen profiles on top of trait codegen.
- Generated bindings are stable enough that source hash churn does not force
  hand edits in host code. **Done by
  [ADR 0095](../decisions/0095-host-callback-trait-codegen.md):** trait
  method names derive from operation labels, not hashes; hosts no longer
  copy hash-derived field names.

### Stage 5: Tacboy Vertical Slice

**Purpose:** Prove the design on a real stateful workload before broadening
the language.

Work items:

- Port Tacboy from scalar toy-kernel shape to a Tacit-owned instance.
- Keep the host limited to ROM file loading, input polling, window/frame
  presentation, audio output, timing, and process lifecycle.
- Implement CPU registers, memory map, instruction decode, timer, interrupt
  state, and cartridge ROM/RAM ownership in Tacit.
- Add PPU framebuffer production using bulk buffer transfer, not scalar
  per-pixel polling.
- Add package tests for pure CPU/decode helpers and host-driven integration
  tests for instance lifecycle.
- Record throughput against the Stage 0 benchmark.

Exit criteria:

- Tacboy can run a meaningful emulator loop with Tacit-owned state.
- The host contains no CPU, PPU, timer, interrupt, or memory-map semantics.
- Performance data identifies whether remaining bottlenecks are language,
  ABI, codegen, or host-library issues.

### Stage 6: Hardening And Freeze

**Purpose:** Turn the experiment into a durable language/toolchain surface.

Work items:

- Audit diagnostics for state escape, invalid instance use, invalid vector
  lifetime, missing destroy, bad borrowed-buffer arguments, and callback
  failures.
- Add inspection overlays for instance fields, persistent vectors, loops, and
  host capabilities.
- Update the primer only after the surface stabilizes, keeping it
  language-facing and independent of repository logistics.
- Add release notes and migration guidance from scalar-only Stage 11 host
  libraries.
- Freeze the accepted subset with an ADR.

Exit criteria:

- CI covers rich boundary codegen, bounded loops, package instances, and
  capability callbacks.
- The primer and workflow assets match the released toolchain hash.
- The freeze ADR states which parts remain future work.

## Open Design Questions

| ID | Question | Resolution Point |
| --- | --- | --- |
| Q-SHB-1 | Is guaranteed self-tail-call lowering enough, or does Tacit need an explicit loop construct? | Closed by [ADR 0093](../decisions/0093-bounded-stack-loop-primitive.md): standalone `@loop` primitive. |
| Q-SHB-2 | What is the canonical representation for package-instance state declarations? | Closed by [ADR 0094](../decisions/0094-stateful-host-bridge-package-instances.md): new `(state name-sym record-ty)` entry inside the unit's `defs` list. |
| Q-SHB-3 | Are Tacit-owned heap vectors a new handle family or an extension of typed vectors? | Closed by [ADR 0094](../decisions/0094-stateful-host-bridge-package-instances.md): reuse existing typed-vec handles; ownership lives in the instance, the user always sees a call-local borrow. |
| Q-SHB-4 | How are allocation failures represented across instance creation and method calls? | Closed by [ADR 0094](../decisions/0094-stateful-host-bridge-package-instances.md): new ABI status `TACIT_STATUS_OUT_OF_MEMORY`; no Tacit-level failure value. |
| Q-SHB-5 | Can host callbacks receive Tacit-owned borrowed slices safely during a call? | Closed by [ADR 0094](../decisions/0094-stateful-host-bridge-package-instances.md): yes, symmetric Stage 1 call-local borrow rule. |
| Q-SHB-6 | Which capability labels belong in a conventional profile versus project-local declarations? | Closed by [ADR 0095](../decisions/0095-host-callback-trait-codegen.md): none. Profiles are not introduced; capability labels remain project-local declarations. May be revisited if a second consumer of the same logical capability emerges. |
| Q-SHB-7 | What is the minimum Tacboy milestone that proves the model without turning Tacit development into emulator development? | Stage 5 plan update |

## Recommended Sequence

The highest-leverage first step is Stage 1. Rich boundary codegen is already
partly designed by the existing host-interface ABI, and it immediately removes
the worst scalar-call pressure for ROM, frame, and audio buffers.

The second step should be Stage 2. Persistent state is not useful for emulator
work if the main execution loop can still grow stack or depend on backend
optimizer behavior.

Only after those two are proven should Tacit add package instances. Instance
state is the largest semantic expansion in this plan, so it should be designed
against working bulk-boundary and loop evidence rather than speculation.
