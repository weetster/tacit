# 0089 - Phase 6 frozen

**Status:** Accepted
**Date:** 2026-05-17
**Phase:** 6 (exit)
**Closes:** [phase-6-plan.md Stage 12](../plans/phase-6-plan.md) and
[phase-6-plan.md Q-P6-15](../plans/phase-6-plan.md)
**Artifacts frozen by this ADR:**
- [decisions/0079-phase-6-scope.md](0079-phase-6-scope.md) - Phase 6
  scope and stage plan.
- [decisions/0080-phase-6-module-semantics.md](0080-phase-6-module-semantics.md) -
  unit imports, exports, visibility, and hash semantics.
- [decisions/0081-phase-6-project-graph.md](0081-phase-6-project-graph.md) -
  whole-project graph and deterministic derived layout.
- [decisions/0082-phase-6-package-manifest-lockfile-cache.md](0082-phase-6-package-manifest-lockfile-cache.md) -
  package manifest, lockfile, dependency cache, and object store.
- [decisions/0083-phase-6-package-tests.md](0083-phase-6-package-tests.md) -
  package tests and structured results.
- [decisions/0084-phase-6-fixed-width-integers.md](0084-phase-6-fixed-width-integers.md) -
  fixed-width integers and bit primitives.
- [decisions/0085-phase-6-typed-mutable-memory.md](0085-phase-6-typed-mutable-memory.md) -
  typed mutable memory.
- [decisions/0086-phase-6-data-layout-and-decode.md](0086-phase-6-data-layout-and-decode.md) -
  data layout and decode support.
- [decisions/0087-phase-6-source-level-stdlib-foundations.md](0087-phase-6-source-level-stdlib-foundations.md) -
  source-level stdlib foundations.
- [decisions/0088-phase-6-host-interface-abi.md](0088-phase-6-host-interface-abi.md) -
  constrained host-interface ABI.
- [plans/phase-6-plan.md](../plans/phase-6-plan.md) - all Phase 6 stages
  complete.
- [plans/primer/tacit-lite-primer.md](../plans/primer/tacit-lite-primer.md) -
  Phase 6 primer baseline at 26,265 `o200k_base` tokens.
- [examples/phase-6/](../examples/phase-6/) - durable Phase 6 examples.
- [stdlib/tacit/](../stdlib/tacit/) - source-level stdlib packages.

## Context

Phase 6 was scoped as the bridge from single-program Tacit-Lite to a real
ecosystem surface: modules, multi-file projects, local hash-addressed
packages, package tests, systems primitives, source-level stdlib foundations,
and a constrained embedding ABI. The host-interface work remained an
embedding ABI, not general FFI.

All design ADRs and implementations for the planned Stage 1 through Stage 11
surface have landed. Stage 12 updated the Tacit-Lite primer and re-baselined
its token count. The remaining Stage 12 choice was whether to run a model or
open-corpus evaluation before freezing Phase 6.

## Delivered surface

Phase 6 delivers the planned ecosystem and systems slice:

- `unit` artifacts with explicit import/export/private boundaries, exact
  definition-hash imports, public/package visibility, host-import entries,
  and type/effect signature checking at boundaries.
- Whole-project graph loading over multiple `.tac` / `.tacd` files with
  deterministic file-layout-independent checking, inspection, and project
  entry expansion.
- Package manifests, lockfiles, hash-indexed dependency cache objects, path
  dependency locking, cache verification, and package-level entry selection.
- Package tests selected from the manifest by definition hash, with `Bool`
  targets, explicit effect policy, and stable `tacit-test-v1` JSON results.
- Fixed-width integer types from `i8`/`u8` through `i64`/`u64`, explicit
  casts, wrapping/checked/saturating arithmetic, bit operations, shifts,
  rotates, masks, and byte-order helpers.
- Typed mutable-memory handles for the fixed-width integer families, including
  length-carrying vectors, bounds-checked access, `u8vec` slices, byte
  operations, and byte-bus load/store helpers.
- Structural record and fixed-width decode examples sufficient for
  emulator-style CPU state, memory bus, and instruction decoder skeletons
  without making a full emulator a Phase 6 deliverable.
- Source-level stdlib packages for core, byte, array, text, collections, and
  IO wrappers, consumed through ordinary package resolution and exact imports.
- Constrained host-interface metadata, C header generation, Rust bindings,
  host-provided imports as typed capability declarations, ownership/lifetime
  rules, result/error ABI, and LLVM-native target selection.
- A Rust embedding demo that links a Tacit kernel as a static library,
  satisfies a host callback, calls public Tacit exports, and runs package
  tests for the pure kernel definitions.

## Regression evidence

The Stage 12 non-evaluation regression checks pass:

```text
cargo test --workspace --features tacit-codegen/llvm19-1,tacit-cli/llvm19-1
```

This includes canonical unit artifacts, project graph, package/lock/cache,
package tests, fixed-width integers, typed memory, data-layout/decode,
source-stdlib package consumption, host-interface metadata/header/bindings,
library codegen, CLI project/package/interface/test flows, primer fixture
validation, and the existing smoke suites.

The embedding kernel package test passes with stable JSON output:

```text
cargo run -p tacit-cli --features llvm19-1 -- \
  test examples/phase-6/embedding-demo/kernel --format json
```

The result was `4` total tests, `4` pass, `0` fail, `0` compile-fail,
`0` effect-fail, and `0` error.

The Rust host embedding demo runs successfully:

```text
cargo run -p tacit-embedding-demo-host
```

The host linked the generated Tacit static library, called `decode-op`,
`step-cpu`, and `log-acc`, satisfied the host callback, observed
`host_log: [40]`, and printed `ok`.

No sealed-corpus path, metadata, content, or feedback was used.

## Evaluation decision

No model/open-corpus evaluation was run for Stage 12.

The Phase 6 primer update is a completeness update, not a token-efficiency
hypothesis. It teaches modules, packages, tests, fixed-width systems
primitives, typed memory, source-level stdlib imports, and the host ABI. That
material increases primer size and cognitive surface. It is not expected to
improve mid-tier model token efficiency, and a model evaluation at this point
would mostly measure the larger recurring primer tax rather than a meaningful
language-density improvement.

The accepted Stage 12 evidence is therefore regression evidence, durable
examples, and the primer baseline, not a new fluency or density run. A future
evaluation should be triggered by a specific fluency, maintenance, or
token-efficiency hypothesis, with a metric decision that separates recurring
primer cost from generated authoring output.

## Decision

**Phase 6 is frozen.**

The phase satisfies its exit criteria: a multi-module Tacit package can be
checked, compiled, tested, dependency-resolved by hash, and consumed by a Rust
host through the constrained embedding ABI. Host-provided capabilities are
visible through explicit type/effect signatures, and the systems primitive
surface is sufficient for emulator-style CPU, memory-bus, and decoder
skeletons.

Q-P6-15 is closed by the durable Phase 6 examples plus the regression evidence
above. The examples and tests prove emulator-style expressiveness without
expanding scope into a full video game emulator or a new open-corpus
evaluation cycle.

Phase 7 may begin.

## Deferrals

The following remain out of scope after Phase 6:

- Arbitrary `extern "C"` declarations from Tacit source.
- Direct bindings to ecosystem libraries, dynamic plugin loading, untyped
  pointer escape hatches, or host allocator hooks.
- Semantic-version solving, public registry operation, and broad package
  tooling.
- WASM as a Phase 6 host target.
- Full debugger, structural diff, blame, merge, Git driver, IDE, and
  registered-view system work.
- Full game emulator, windowing, audio, input, ROM/file selection, timing, and
  platform integration inside Tacit.
- Row polymorphism, user-defined effects, effect handlers, capabilities,
  refinement types, and concurrency.
- Performance hardening for systems code, including optimizer pipelines,
  tighter closure-environment lowering, and scalar replacement.
- Static-library codegen for non-scalar host-boundary types. Phase 6 interface
  metadata, C headers, and Rust bindings cover records and borrowed typed
  vectors, but the linkable-library codegen subset remains scalar-only and
  rejects unsupported boundaries with diagnostics.

## Consequences

- Phase 6 artifacts are accepted as the stable baseline for Phase 7 tooling.
- Future work should design debugging, diff, blame, merge, IDE, and broad
  package-tooling features against real unit/package/test/host-interface
  boundaries rather than simulated single-file programs.
- Primer growth is recorded as a cost. More primer prose should not be added
  for density reasons without a specific hypothesis and metric decision.
- The sealed-corpus boundary remains intact.
