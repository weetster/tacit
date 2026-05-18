# 0091 - Stateful host-bridge scope

**Status:** Accepted
**Date:** 2026-05-18
**Phase:** Stateful host-bridge, Stage 0 scope
**Closes:** [stateful-host-bridge-plan.md Stage 0](../plans/stateful-host-bridge-plan.md)

## Context

Phase 6 delivered the constrained host-interface ABI, generated interface
metadata, C headers, Rust bindings, scalar linkable library codegen,
fixed-width integers, typed mutable vectors, and emulator-style CPU/memory
examples. It deliberately did not deliver a full emulator, persistent
Tacit-owned state across host calls, rich non-scalar library boundary codegen,
or performance hardening for long-running systems code.

Tacboy exposes the next pressure point. The desired architecture is not a Rust
host that owns emulator semantics and calls small Tacit helper functions. The
desired architecture is a Tacit package that owns CPU, memory, video, audio,
timer, and interrupt state, while the host only bridges typed Tacit
capabilities to third-party libraries such as windowing, audio, input,
storage, and timing.

The existing host model is still the right boundary:

- Tacit source declares typed host imports with concrete effects.
- The host satisfies those imports through generated callback tables.
- Tacit source does not name arbitrary C symbols or third-party library entry
  points.
- Host-owned platform resources remain outside the Tacit value graph.

The gap is that the current implementation forces scalar-only linkable
exports and imports, stack-scoped Tacit vectors, and recursion-based
long-running loops. That makes a full emulator possible only by moving too
much state and semantic responsibility into the host.

No design, implementation, benchmark, or validation work for this track may
read, list, search, or otherwise depend on `corpus/sealed/`.

## Decision

Create a bounded stateful host-bridge track as future Tacit language and
toolchain work.

The track's goal is to let Tacit own long-running application state while the
host remains a bridge to platform and third-party library capabilities. This
is explicitly not general FFI.

The accepted stage sequence is:

1. Complete rich boundary library codegen for ABI records and borrowed typed
   vector parameters.
2. Add a bounded-stack loop contract, either through explicit loop syntax or
   mechanically guaranteed self-tail-call lowering.
3. Add Tacit-owned package instances with an explicit create/call/destroy
   lifecycle and persistent owned state.
4. Define optional conventional host capability profiles for video, audio,
   input, monotonic time, logging, and storage.
5. Prove the model with a Tacboy vertical slice where Tacit owns emulator
   state and the host contains no emulator-domain semantics.
6. Harden diagnostics, inspection, primer text, release notes, and freeze the
   accepted subset.

Tacboy is accepted as an open exemplar and stress test. It is not a hidden
benchmark, not a sealed evaluation source, and not a reason to special-case
Game Boy concepts in the language. Emulator-specific state and behavior should
live in Tacboy or ordinary Tacit packages; generic host-bridge mechanisms
belong in the language/toolchain.

Stage 0 also accepts the benchmark shape in
[stateful-host-bridge-benchmark.md](../plans/stateful-host-bridge-benchmark.md).
The benchmark must measure CPU-step throughput, framebuffer transfer,
audio-buffer transfer, host-call overhead, bounded-stack loop behavior, and
host/domain responsibility separation. Initial numeric throughput results are
descriptive until a later ADR sets calibrated thresholds. Structural results
such as "bulk transfer does not require one ABI call per pixel/sample" and
"host code contains no emulator-domain state" may be gates.

## Non-goals

- No arbitrary `extern "C"` declarations from Tacit source.
- No direct Tacit bindings to SDL, OpenGL, Vulkan, ALSA, CoreAudio, POSIX,
  Win32, or other ecosystem APIs.
- No untyped pointer escape hatches.
- No unchecked memory access by default.
- No ambient mutable globals outside an explicit package-instance ownership
  model.
- No semantic-version package solving or public registry work.
- No concurrency, async runtime, actor model, or scheduler in this track.
- No WASM backend commitment without a later target-specific ADR.
- No sealed-corpus development feedback.

## Alternatives considered

### Keep emulator state in the host

Rejected. It works with the current scalar ABI, but it turns Tacit into a
helper-kernel language for this use case. The stated goal is for Tacit to own
domain state and semantics while the host bridges libraries.

### Add general FFI

Rejected. General `extern "C"` would undermine content-addressed capability
declarations, generated ABI metadata, ownership rules, and the existing
host-kernel separation. The bridge should remain typed and generated.

### Add mutable globals only

Rejected. Mutable globals would solve persistence but not ownership,
lifecycle, multiple emulator instances, destruction, or host boundary rules.
Persistent state should be attached to explicit package instances.

### Make Tacboy the language spec

Rejected. Tacboy is useful because it stresses state, loops, memory, and bulk
I/O. Those pressures should produce general mechanisms, not Game Boy-specific
syntax or primitives.

## Consequences

- Future work has a bounded path from Phase 6 scalar embedding to
  Tacit-owned long-running state.
- Rich boundary codegen and bounded loops should land before package
  instances, because they reduce risk before the largest semantic expansion.
- Host capability profiles are optional conventions, not privileged library
  bindings.
- Benchmark evidence must separate structural viability from raw performance.
- The sealed-corpus boundary remains intact.
