# Phase 6 Plan

**Status:** Active; Stage 0 complete by
[ADR 0079](../decisions/0079-phase-6-scope.md)
**Scope:** Modules, packages, systems primitives, unit testing, source-library
foundations, dependency caching, and the constrained host-interface ABI

## Context

Phase 5 is complete. [ADR 0078](../decisions/0078-phase-5-decision.md)
accepted the decision to proceed to Phase 6 without a pre-Phase-6 maintenance
tool spike. The accepted handoff is narrow: Phase 6 should focus on modules,
packages, systems primitives, and the constrained host-interface ABI. Full
debugger, diff, blame, merge, IDE, and broad package-tooling work remains
Phase 7 unless a later bounded ADR reopens one narrow tool with new evidence.

Phase 6 is the bridge between the current single-program research artifact and
a real Tacit ecosystem. It makes Tacit code composable across definitions,
files, packages, low-level systems components, and non-Tacit host programs
without abandoning content-addressed identity.

The host-interface work is an embedding ABI, not general FFI. Tacit modules
declare typed imports and exports; a C or Rust host satisfies imports and calls
exported Tacit logic. Tacit source does not get arbitrary `extern "C"` escape
hatches, direct bindings to random ecosystem libraries, dynamic plugin loading,
or untyped pointer escape hatches.

No Phase 6 work may read, list, search, or otherwise access
`corpus/sealed/`. Sealed grading, if ever requested, is an operator-triggered
evaluation action and must not provide development feedback.

## Stage 0 Outcome

Stage 0 scope lock is complete. Phase 6 is active under this plan.

Locked decisions:

- Phase 6 is modules, packages, systems primitives, unit testing,
  source-library foundations, dependency caching, and constrained embedding
  ABI work.
- Phase 5 is complete and does not require a pre-Phase-6 tool spike.
- General FFI remains out of scope: no arbitrary `extern "C"`, direct
  ecosystem-library bindings, dynamic plugin loading, or untyped pointer
  escape hatches.
- Package work is local and hash-based: no semantic-version solver and no
  public registry operation in Phase 6.
- Full debugger, structural diff, blame, merge, Git driver, IDE, and broad
  package tooling remain Phase 7.
- A full video game emulator is not a Phase 6 deliverable; Phase 6 must only
  prove emulator-style expressiveness for a CPU core, memory bus, and
  instruction decoder skeleton.
- Q-P6-1 through Q-P6-15 and the required ADR sequence below are the active
  resolution map.
- No Phase 6 development work may use sealed-corpus contents, paths, metadata,
  or feedback.

## Goal

Enable a multi-module Tacit package to be checked, compiled, tested, resolved
by hash, and consumed from a C or Rust host through a constrained embedding ABI.
The systems primitive surface should be sufficient to express an
emulator-style CPU core, memory bus, and instruction decoder in Tacit, while
performance hardening remains Phase 8.

## Deliverables

1. Module semantics for exports, imports, explicit type/effect signatures,
   content-hash identity, sidecar display aliases, mutual-recursion group
   boundaries, and imported-hash type/effect checking.
2. Multi-file project support for multiple `.tac`/`.tacd` units while
   preserving the rule that file layout has no semantic weight.
3. Project-level CLI commands that check, compile, and inspect the whole graph.
4. A local package model with a manifest and lockfile that refer to dependency
   hashes, not semantic-version ranges.
5. A local hash-indexed dependency cache and object store for package and
   definition artifacts.
6. Unit testing support that calls exported definitions, runs package-level
   tests, and emits structured results.
7. Fixed-width signed and unsigned integer types from `i8`/`u8` through
   `i64`/`u64`.
8. Explicit numeric casts, truncation, sign extension, zero extension,
   wrapping arithmetic, checked arithmetic, saturating arithmetic, bitwise
   operations, shifts, rotates, masks, and byte-order helpers.
9. Typed mutable memory beyond today's `Buf` and `I64Vec`: byte-addressable
   arrays/slices, typed arrays where needed, explicit bounds behavior,
   slice/view operations, and read/write effect signatures.
10. Data-layout and decode support sufficient for CPU/device state and
    instruction decoding without pulling in Tacit-Full refinements.
11. Source-level stdlib foundations for strings, collections, typed arrays,
    byte-order helpers, file I/O helpers, and source-defined wrappers around
    existing primitives.
12. A stable constrained host-interface ABI with generated interface metadata,
    C headers, Rust host bindings, host-provided imports, ownership/lifetime
    rules, result/error ABI, allocator-boundary rules, and capability/effect
    declarations.
13. A small C or Rust embedding demo that proves the "Tacit logic kernel inside
    a conventional host" model.
14. Primer updates for the Phase 6 language and package surface, kept
    language-facing and free of repository logistics.
15. A Phase 6 freeze ADR.

## Non-Goals

- No arbitrary `extern "C"` declarations from Tacit source.
- No direct Tacit bindings to SDL, OpenGL, SQLite, OpenSSL, libcurl, or other
  arbitrary ecosystem libraries.
- No untyped pointer escape hatches.
- No unsafe unchecked memory access by default.
- No dynamic plugin loading.
- No semantic-version dependency solving.
- No public package registry operation.
- No HTTP or networking as built-in language primitives.
- No full video game emulator as a Phase 6 deliverable.
- No full debugger, structural diff, blame, merge, Git driver, or IDE work.
- No row polymorphism, user-defined effects, effect handlers, capabilities,
  refinement types, or concurrency.
- No Python-relative density gate.
- No sealed-corpus development feedback.

Windowing, audio, input, ROM/file selection, timing, and platform integration
remain host-owned capabilities during Phase 6.

## Open Questions

| ID | Question | Resolution Point |
| --- | --- | --- |
| Q-P6-1 | How are imports and exports represented in canonical form and authoring view? | Resolved by [ADR 0080](../decisions/0080-phase-6-module-semantics.md) |
| Q-P6-2 | What is public, package-local, or private at module boundaries? | Resolved by [ADR 0080](../decisions/0080-phase-6-module-semantics.md) |
| Q-P6-3 | How do imported hashes participate in type/effect checking and diagnostics? | Resolved by [ADR 0080](../decisions/0080-phase-6-module-semantics.md) |
| Q-P6-4 | What project layout is deterministic while keeping file layout non-semantic? | Resolved by [ADR 0081](../decisions/0081-phase-6-project-graph.md) |
| Q-P6-5 | What manifest and lockfile formats represent hash-based dependencies? | Resolved by [ADR 0082](../decisions/0082-phase-6-package-manifest-lockfile-cache.md) |
| Q-P6-6 | What local object-store layout and cache invalidation rules are required? | Resolved by [ADR 0082](../decisions/0082-phase-6-package-manifest-lockfile-cache.md) |
| Q-P6-7 | What is the minimum unit-test surface and structured result schema? | Resolved by [ADR 0083](../decisions/0083-phase-6-package-tests.md) |
| Q-P6-8 | Are fixed-width integer operations primitives, source-library functions, or both? | Resolved by [ADR 0084](../decisions/0084-phase-6-fixed-width-integers.md) |
| Q-P6-9 | What typed mutable-memory surface replaces or subsumes `Buf` and `I64Vec`? | Resolved by [ADR 0085](../decisions/0085-phase-6-typed-mutable-memory.md) |
| Q-P6-10 | Are existing records, constructors, and `match` sufficient for decode shapes? | Resolved by [ADR 0086](../decisions/0086-phase-6-data-layout-and-decode.md) |
| Q-P6-11 | Which compiler-recognized primitives move first into source-level stdlib packages? | Resolved by [ADR 0087](../decisions/0087-phase-6-source-level-stdlib-foundations.md) |
| Q-P6-12 | What Tacit types are ABI-expressible at the host boundary? | Resolved by [ADR 0088](../decisions/0088-phase-6-host-interface-abi.md) |
| Q-P6-13 | What ownership, lifetime, allocation, and result/error rules govern host calls? | Resolved by [ADR 0088](../decisions/0088-phase-6-host-interface-abi.md) |
| Q-P6-14 | Does Phase 6 commit only to LLVM-native linkable artifacts, or also to WASM? | Resolved by [ADR 0088](../decisions/0088-phase-6-host-interface-abi.md) |
| Q-P6-15 | What examples and benchmarks prove emulator-style expressiveness without becoming a full emulator? | Stage 12 freeze ADR |

## Required ADR Sequence

ADRs must land before implementation that depends on them.

1. Phase 6 scope and stage plan. Done:
   [ADR 0079](../decisions/0079-phase-6-scope.md).
2. Module imports, exports, visibility, and imported-hash semantics. Done:
   [ADR 0080](../decisions/0080-phase-6-module-semantics.md).
3. Multi-file project graph and deterministic derived layout. Done:
   [ADR 0081](../decisions/0081-phase-6-project-graph.md).
4. Package manifest, lockfile, dependency cache, and object-store layout.
   Done:
   [ADR 0082](../decisions/0082-phase-6-package-manifest-lockfile-cache.md).
5. Package-level test surface and structured test-result schema. Design
   accepted by [ADR 0083](../decisions/0083-phase-6-package-tests.md);
   implementation complete.
6. Fixed-width integer and bit-operation surface. Done:
   [ADR 0084](../decisions/0084-phase-6-fixed-width-integers.md).
7. Typed mutable memory and bounds behavior. Done:
   [ADR 0085](../decisions/0085-phase-6-typed-mutable-memory.md).
8. Data layout and decode support. Done:
   [ADR 0086](../decisions/0086-phase-6-data-layout-and-decode.md).
9. Source-level stdlib migration path. Design accepted by
   [ADR 0087](../decisions/0087-phase-6-source-level-stdlib-foundations.md);
   implementation complete.
10. Host-interface ABI, ABI-expressible type subset, ownership, allocation,
    result/error handling, and backend target decision. Design accepted by
    [ADR 0088](../decisions/0088-phase-6-host-interface-abi.md);
    implementation planned.
11. Phase 6 freeze.

## Stage 0: Scope Lock

**Status:** Complete 2026-05-10. Deliverable:
[ADR 0079](../decisions/0079-phase-6-scope.md)

**Purpose:** Turn this plan into the binding Phase 6 scope artifact before
implementation begins.

Work items:

- Confirm Phase 6 is modules, packages, systems primitives, unit testing,
  source-library foundations, dependency caching, and constrained embedding
  ABI work.
- Confirm Phase 5 is complete and does not require a pre-Phase-6 tool spike.
- Lock non-goals, especially no arbitrary FFI, no untyped pointers, no semver
  solver, no public registry, no full debugger, and no full emulator.
- List all Phase 6 open questions with resolution stages.
- Define the required ADR sequence.
- Preserve the sealed-corpus boundary.

Outcome:

- Phase 6 scope is accepted by [ADR 0079](../decisions/0079-phase-6-scope.md).
- The Phase 5 handoff from [ADR 0078](../decisions/0078-phase-5-decision.md)
  is preserved: no pre-Phase-6 maintenance tool spike.
- Non-goals and Phase 7 boundaries are locked.
- Q-P6-1 through Q-P6-15 are listed with resolution stages.
- The required ADR sequence is listed above and blocks dependent
  implementation.
- The sealed-corpus boundary remains active.

Exit criteria:

- `plans/phase-6-plan.md` is accepted as the active working plan.
- Q-P6-1 through Q-P6-15 are listed with resolution points.
- No implementation work is blocked on missing scope text.
- No Phase 6 plan text requires access to `corpus/sealed/`.

## Stage 1: Module Semantics

**Status:** Complete 2026-05-13. Design ADR accepted 2026-05-10;
implementation verified in canonical, view, and typechecker tests.

**Purpose:** Specify Tacit-to-Tacit module composition before project, package,
stdlib, or host-interface work depends on it.

Work items:

- Write the module-semantics ADR.
- Specify import and export syntax in authoring view.
- Specify canonical representation for import and export metadata.
- Decide public, package-local, and private visibility rules.
- Define explicit type/effect signature requirements at module boundaries.
- Define content-hash identity for exported definitions.
- Define local display aliases in sidecar metadata.
- Define mutual-recursion group boundaries across modules.
- Define how imported hashes participate in type/effect checking.
- Define diagnostics for missing imports, hash mismatches, signature
  mismatches, visibility violations, and cyclic dependency errors.
- Specify inspection-view rendering for imports, exports, and imported hashes.
- Add canonical, authoring, sidecar, and diagnostic test-vector expectations.

Outcome:

- Module semantics are accepted by
  [ADR 0080](../decisions/0080-phase-6-module-semantics.md).
- Canonical `unit`, `imports`, `imp`, `exports`, `exp`, `defs`, `def`,
  `sig`, and `ref` artifacts parse and emit deterministically.
- Authoring view supports `unit`, `import ... from blake3:<hash>`,
  `export public`, `export package`, and `private` declarations.
- Sidecar metadata carries unit, import, definition, and export display
  aliases; stale duplicate aliases fall back to synthetic hash-based names.
- Inspection view renders import/export/private boundaries and hash references.
- The checker resolves imported definitions by exact hash, verifies declared
  signatures, checks visibility, rejects dangling and duplicate boundary
  entries, and detects hash-reference dependency cycles. Unit diagnostics use
  unambiguous sidecar aliases alongside hashes when alias metadata is available.
- A single in-memory package can contain multiple logical units through
  `check_units_in_memory`.
- Direct tests cover canonical ordering, authoring syntax, inspection output,
  sidecar alias fallback, imported-hash checking, and all Stage 1 reserved
  diagnostic kinds.
- Whole-project loading, file discovery, project-level check/compile, and
  deterministic multi-file graph traversal remain Stage 2.

Exit criteria:

- The module-semantics ADR is accepted.
- A single package can contain multiple logical modules in memory.
- The checker can resolve an imported definition by hash and verify its
  declared type/effect signature.
- Inspection output exposes import/export boundaries clearly enough for later
  Phase 7 tooling.
- No cross-module behavior depends on file path ordering.

## Stage 2: Whole-Project Graph

**Status:** Complete 2026-05-14. Design ADR accepted by
[ADR 0081](../decisions/0081-phase-6-project-graph.md); implementation
verified for deterministic project loading, local hash indexing, derived
layout materialization, project-level `check`, project inspection, and
project-level `compile` for standalone executable entries.

**Purpose:** Make multi-file projects real while preserving the rule that file
layout has no semantic weight.

Work items:

- Write the project-graph ADR if Stage 1 leaves project layout undecided.
- Define the project root discovery rule.
- Define how multiple `.tac`/`.tacd` units are loaded.
- Define the deterministic derived layout for generated outputs.
- Add a local hash index for definitions and modules.
- Add graph construction with deterministic traversal and diagnostics.
- Add project-level `check` support.
- Add project-level `compile` support for standalone executables.
- Add project-level `view` or inspection entry points where useful.
- Extend existing round-trip and sidecar tests to multi-file fixtures.
- Add negative tests for duplicate aliases, missing sidecars, stale sidecars,
  unresolved imports, and dependency cycles.

Exit criteria:

- A project with multiple `.tac`/`.tacd` units can be checked as one graph.
- A project with multiple modules can be compiled deterministically.
- The same semantic graph hashes identically regardless of file ordering.
- Diagnostics identify the logical module/import problem without treating file
  layout as semantic.

Outcome:

- Project roots load canonical `unit` artifacts from `src/` when present, or
  from the root otherwise; `.taca`, `.tacd`, `.git`, `.tacit`, and `target`
  are not semantic project inputs.
- The loader builds a deterministic hash-ordered project graph, coalesces
  duplicate unit and definition artifacts by hash, preserves fresh sidecar
  aliases, and ignores missing or stale sidecars.
- Project-level `check` resolves same-package imports through the local hash
  index and reports missing imports, visibility failures, and malformed unit
  artifacts without using file path order as meaning.
- Project-level `view --as inspection` renders a graph summary plus per-unit
  inspection views.
- Project-level `compile` selects a public export by `blake3:<hash>`, raw
  hash, sidecar alias, or the sole public export; it lowers the selected
  standalone `Int`/`Bool` entry by expanding local hash refs and writes
  deterministic derived artifacts under `.tacit/derived/project-<hash>/`.
- Tests cover deterministic file-order independence, missing and stale
  sidecars, duplicate entry aliases, unresolved imports, private visibility
  violations, dependency-cycle rejection during entry lowering, derived layout
  materialization, project inspection, and LLVM IR generation for a multi-unit
  project entry.

## Stage 3: Package Manifest, Lockfile, And Cache Design

**Status:** Complete 2026-05-13. Design ADR accepted by
[ADR 0082](../decisions/0082-phase-6-package-manifest-lockfile-cache.md).

**Purpose:** Specify hash-based package composition before implementation
commits to file formats or cache layout.

Work items:

- Write the package/cache ADR.
- Define the package manifest format.
- Define the lockfile format.
- Define package identity by content hash.
- Define dependency references by hash, not semantic-version ranges.
- Define optional registry alias behavior as name-to-hash lookup only.
- Define local path dependencies for development.
- Define the hash-indexed object-store layout.
- Define cache fetch, verification, eviction, and corruption diagnostics.
- Define compatibility between package manifests and the local project graph.
- Define how sidecars and interface metadata are stored in package artifacts.

Exit criteria:

- Package manifest and lockfile formats are accepted.
- Cache/object-store invariants are accepted.
- Registry operation is explicitly out of scope.
- Historical dependency hashes are buildable without relying on mutable names.

Outcome:

- Package identity is content-addressed by the same byte sequence as the
  project-graph hash (ADR 0081), recomputed under the `tacit-package-v1`
  envelope tag. Manifest display fields and declared dependency aliases do
  not change package identity.
- The manifest is `tacit.toml`. Schema accepts optional `[package]`,
  `[dependencies]`, `[exports]`, and `[bin]` tables. Dependencies pin
  `hash = "blake3:<hex>"` or `path = "<relative>"`; combined or missing
  sources are rejected with structured diagnostics. Registry hints are
  passive metadata.
- The lockfile is `tacit.lock`, a deterministic JSON file pinning the direct
  and transitive dependency closure. Keys are fixed-order, entries are
  sorted, and serialization round-trips byte-exactly.
- The dependency cache lives at `.tacit/cache/` with a hash-rooted layout
  (`objects/units`, `objects/defs`, `objects/sidecars`, `packages/<hash>/`).
  Reads recompute BLAKE3 and reject corruption; writes are atomic; eviction
  is explicit. Stage 10 reserves `packages/<hash>/interface.json`.
- Lockfile drift, manifest schema errors, dependency-resolution failures,
  cache corruption, and circular package dependencies have reserved
  structured diagnostic kinds for Stage 4 implementation.
- Project-graph compatibility is preserved: a manifestless project is a
  valid manifestless package whose hash equals its project-graph hash under
  the new envelope tag.

## Stage 4: Package And Dependency Implementation

**Status:** Complete 2026-05-15. Implementation verified for manifest and
lockfile parsing, local path dependencies, hash-only cache dependencies,
package-level `check`, package-level `compile` entry resolution, cache object
verification, lockfile drift, and deterministic lock regeneration.

**Purpose:** Land the package model as an end-to-end compiler and CLI slice.

Work items:

- Implement manifest and lockfile parsing.
- Implement lockfile verification against dependency hashes.
- Implement local path dependencies.
- Implement the hash-indexed object store.
- Implement package graph construction.
- Implement package-level `check`.
- Implement package-level `compile`.
- Add structured diagnostics for manifest, lockfile, cache, and dependency
  failures.
- Add fixtures for local package dependencies.
- Add tests for cache hits, cache misses, hash mismatches, missing artifacts,
  and deterministic rebuilds.

Exit criteria:

- A local package can depend on another local package by hash.
- `check` and `compile` work from manifest plus lockfile.
- The dependency cache verifies artifact hashes before use.
- Build output is deterministic for a fixed lockfile and source set.

Outcome:

- `tacit.toml` parsing accepts strict `[package]`, `[dependencies]`,
  `[exports]`, and `[bin]` tables, rejects unknown fields, ambiguous
  dependency sources, missing dependency sources, malformed hashes, and
  unresolved package entries with the reserved Stage 4 diagnostics.
- `tacit.lock` is emitted as deterministic JSON by `tacit lock`, verified by
  package-aware directory `check` and `compile`, and rejects drift from path
  dependency mutation, manifest edits, malformed lockfiles, or missing
  lockfiles for packages with dependencies.
- Local path dependencies are resolved by loading the target project graph,
  computing its `tacit-package-v1` hash, checking the locked hash, and
  materializing its units, definitions, sidecars, manifest snapshot, and
  package index into the consumer's `.tacit/cache/`.
- Hash-only dependencies resolve from `.tacit/cache/packages/<hash>/` and can
  be checked without consulting a path target once the cache is populated.
- The cache writes `objects/units`, `objects/defs`, `objects/sidecars`, and
  `packages/<hash>/package.json`; reads recompute BLAKE3 for unit and
  definition objects, reject corrupt objects, and quarantine tampered files
  under `.tacit/cache/trash/`.
- Package-level `check` extends the Stage 2 project checker with dependency
  definitions while preserving package visibility: external public exports
  import, external package/private definitions produce visibility diagnostics.
- Package-level `compile` resolves entries through `[bin]`, `[exports]`,
  public export hashes, or sidecar aliases, expands dependency hash refs, and
  materializes deterministic derived output under the package hash.
- `tacit cache clear` and `tacit cache evict <hash>` provide the explicit
  cache operations reserved by ADR 0082.
- Tests cover path dependency locking, hash dependency cache reuse, manifest
  `[bin]` entry expansion, lockfile drift, missing cache artifacts, cache
  corruption quarantine, deterministic lock regeneration, manifest
  diagnostics, and CLI package lock/check.

## Stage 5: Unit Testing

**Status:** Design accepted 2026-05-15 by
[ADR 0083](../decisions/0083-phase-6-package-tests.md); implementation
complete 2026-05-16.

**Purpose:** Give multi-module packages a first-class executable test surface
before the systems and host-interface examples grow large.

Work items:

- Write the unit-testing ADR. Done:
  [ADR 0083](../decisions/0083-phase-6-package-tests.md).
- Decide whether tests are ordinary exported definitions, marked test modules,
  manifest entries, or a small test harness convention. Done: tests are
  ordinary package definitions listed from optional `[[tests]]` manifest
  entries by definition hash.
- Define test function signatures and allowed effects. Done: runnable tests
  are zero-input `Bool` definitions with explicit per-entry allowed effects;
  `Div` is not permitted for runnable package tests.
- Define how tests call exported definitions across module and package
  boundaries. Done: tests use normal `unit` imports, package visibility, and
  dependency public exports.
- Define structured test-result JSON. Done: `tacit-test-v1` result envelope
  with deterministic ordering and ADR 0041 diagnostics.
- Add `tacit test` or an equivalent package-level test command. Defined by
  ADR 0083; implemented as `tacit test [ROOT] [--format text|json]`.
- Add pass, fail, panic/error, compile-fail, and effect-fail result cases.
  Defined by ADR 0083; implemented and covered by CLI tests.
- Add examples for pure tests and effectful tests. Done under
  `examples/phase-6/package-tests/`.
- Ensure test output is stable and suitable for AI repair loops. Done in
  design: stable JSON omits timings, absolute paths, and raw process output.

Exit criteria:

- Package-level tests run from the CLI.
- Test results are emitted in a structured stable format.
- Tests can call exported definitions from multiple modules.
- CI covers at least one multi-module package test fixture.

Outcome:

- `tacit.toml` accepts optional strict `[[tests]]` entries with required
  `name`, required local `target = "blake3:<hash>"`, and optional sorted
  `effects = ["Alloc", "IO", "Mut"]`; duplicate test names and targets use
  the reserved `duplicate-test-alias` and `duplicate-test-target`
  diagnostics.
- `tacit test [ROOT] [--format text|json]` loads the same package graph as
  package-level `check` and `compile`, validates local test targets, enforces
  the no-`Div` and allowed-effects policy, lowers selected local definitions
  by hash, and runs executable `Bool` tests.
- JSON output uses the stable `tacit-test-v1` envelope with package metadata,
  summary counters, package-level ADR 0041 diagnostics, deterministic
  hash/name result ordering, per-test declared and allowed effects,
  `observed.bool`, and per-result diagnostics.
- Result statuses cover `pass`, `fail`, `compile-fail`, `effect-fail`, and
  `error`; exit codes follow ADR 0083 (`0` pass, `1` Bool failure only, `2`
  static/runtime/package errors).
- Derived test outputs live under
  `.tacit/derived/project-<package-hash>/tests/`, including `results.json`
  and `build/` intermediates; cache/object-store package identity remains
  untouched.
- Tests cover manifest parsing and diagnostics, a multi-module package test
  calling a package-visible definition, Bool false failures, signature
  compile-fail, effect-fail, and runtime-error JSON.

## Stage 6: Fixed-Width Integer And Bit Primitive Surface

**Status:** Complete 2026-05-16. Design accepted by
[ADR 0084](../decisions/0084-phase-6-fixed-width-integers.md);
implementation verified for fixed-width types, casts, wrapping arithmetic,
checked and saturating add/sub, bit operations, shifts, rotates, masks,
byte-order helpers, inspection rendering through existing symbol paths, and
opcode-style examples.

**Purpose:** Add the numeric surface needed for systems-style Tacit programs
without introducing untyped low-level escapes.

Work items:

- Write the fixed-width integer ADR. Done:
  [ADR 0084](../decisions/0084-phase-6-fixed-width-integers.md).
- Add signed and unsigned integer types: `i8`, `u8`, `i16`, `u16`, `i32`,
  `u32`, `i64`, and `u64`.
- Define literal typing and defaulting rules.
- Define explicit casts, truncation, sign extension, and zero extension.
- Define wrapping, checked, and saturating arithmetic.
- Define bitwise `and`, `or`, `xor`, and `not`.
- Define shifts, rotates, masks, and byte-order helpers.
- Decide which operations are compiler primitives and which are source-level
  stdlib wrappers.
- Extend parser, canonical representation, views, typechecker, and codegen.
- Add structured diagnostics for invalid casts, width mismatches, signedness
  mismatches, shift widths, and checked-operation result handling.
- Add examples that use fixed-width arithmetic for register and opcode work.

Exit criteria:

- Fixed-width integer programs typecheck, compile, inspect, and run.
- Checked operations expose success/failure explicitly.
- No operation requires untyped pointer or unchecked memory access.
- At least one instruction-decode-style example uses the new numeric surface.

Outcome:

- Fixed-width integer types `i8`, `u8`, `i16`, `u16`, `i32`, `u32`,
  `i64`, and `u64` are accepted in type position through existing canonical
  `sym` nodes; `Int` remains the legacy default signed scalar for existing
  programs.
- Integer literals are contextually typed: they may default to a fixed-width
  type only when they fit, otherwise the checker emits
  `integer-literal-out-of-range`; explicit wrapping uses
  `@<ty>-from-int-wrap`.
- Compiler-recognized pure primitives cover explicit truncation, sign
  extension, zero extension, wrapping add/sub/mul, checked add/sub with
  `{ok: Bool, value: <ty>}`, saturating add/sub, bitwise and/or/xor/not,
  shifts, rotates, low-bit masks, byte assembly, and byte swap.
- Typechecking rejects silent use of legacy arithmetic on fixed-width values
  and emits `invalid-shift-width` for statically invalid shift counts.
- Codegen lowers fixed-width values through normalized `i64` representations:
  signed values are sign-extended and unsigned values are zero-extended.
  Dynamic out-of-range shifts have deterministic lowering rather than LLVM
  undefined behavior.
- Authoring and inspection views render fixed-width type names and primitive
  symbols through the existing type and `@name` paths.
- `examples/phase-6/fixed-int/opcode-decode.tac` demonstrates an
  instruction-decode-style low/high nibble kernel with the new numeric surface.

## Stage 7: Typed Mutable Memory

**Status:** Complete 2026-05-16. Design accepted by
[ADR 0085](../decisions/0085-phase-6-typed-mutable-memory.md); implementation
verified for the eight typed-vector types, uniform alloc/len/get/set surface,
`u8vec` fill/copy/slice/eq/scan, byte-bus typed loads and stores, bounds-trap
semantics, anti-escape diagnostics, and end-to-end smoke executables.

**Purpose:** Replace the ad hoc `Buf` and `I64Vec` era with a clear typed
mutable-memory story suitable for package and host-facing code.

Work items:

- Write the typed-memory ADR. Done:
  [ADR 0085](../decisions/0085-phase-6-typed-mutable-memory.md).
- Define byte-addressable arrays and slices.
- Define typed arrays where required by systems examples.
- Define ownership and aliasing restrictions for mutable memory handles.
- Define bounds behavior for reads, writes, slices, and views.
- Define read/write effect signatures.
- Define allocation and deallocation behavior inside Tacit.
- Define how existing `Buf` and `I64Vec` primitives migrate, remain as
  compatibility shims, or are deprecated.
- Extend typechecker escape diagnostics for memory handles.
- Extend codegen for typed arrays and slices.
- Add examples for memory bus reads/writes and byte-slice decoding.

Exit criteria:

- Typed arrays and slices can be allocated, read, written, sliced, and passed
  through module boundaries with explicit effects.
- Bounds behavior is deterministic and documented.
- Existing smoke and Phase 4 examples continue to pass.
- Emulator-style memory-bus examples do not need unsafe unchecked access.

Outcome:

- Tacit gains eight typed-vector handle types `i8vec`, `u8vec`, `i16vec`,
  `u16vec`, `i32vec`, `u32vec`, `i64vec`, `u64vec`, length-carrying in the
  runtime handle with no type-level length parameter.
- Every typed vector exposes a uniform pure/Alloc/Mut surface:
  `@<ty>vec-alloc` (`{Alloc}`), `@<ty>vec-len` (pure), `@<ty>vec-get`
  (pure), `@<ty>vec-set` (`{Mut}`). `u8vec` adds `@u8vec-fill`,
  `@u8vec-copy`, `@u8vec-slice`, `@u8vec-eq`, and `@u8vec-scan`.
- `u8vec` carries twelve byte-bus typed load/store helpers covering
  `u16`/`u32`/`u64` × little-/big-endian.
- All accesses are bounds-checked; out-of-range access invokes
  `llvm.trap` rather than producing undefined behavior. Bounds violation is
  not represented in the effect lattice, matching Stage 6's stance on
  overflow.
- Vec handles are non-escapable: typecheck `invalid-capture` diagnostics
  generalize to all eight new types, and codegen rejects the handle in
  first-class value position. Slices are themselves `u8vec` handles and
  inherit the anti-escape rule.
- All `@<ty>vec-alloc` and `@u8vec-slice` lower as direct `let`-RHS
  stack allocations / sub-views; codegen rejects use outside that position.
- `Buf` and `I64Vec` and their legacy primitives remain unchanged for
  backward compatibility; Phase 1–5 examples continue to compile and run.
- Typecheck fixtures (`crates/tacit-typecheck/tests/stdlib_typed_memory.rs`)
  cover positive cases, byte-bus loads, slice typing, and the two new
  diagnostics. Codegen smoke fixtures
  (`crates/tacit-codegen/tests/p6_typed_memory.rs` plus
  `examples/smoke/p6-u8vec-*.tac` and `examples/smoke/p6-u32vec-*.tac`)
  exercise the round-trip end-to-end.
- Durable Stage 7 examples live under `examples/phase-6/typed-memory/`.

## Stage 8: Data Layout And Decode Support

**Status:** Complete 2026-05-16. Design accepted by
[ADR 0086](../decisions/0086-phase-6-data-layout-and-decode.md);
implementation verified with CPU-state and opcode-decode record examples.

**Purpose:** Decide whether the Phase 4 structural surface is enough for
systems decode shapes, and add only the minimum missing surface.

Work items:

- Write the data-layout/decode ADR. Done:
  [ADR 0086](../decisions/0086-phase-6-data-layout-and-decode.md).
- Evaluate records for CPU registers, flags, and device state.
- Evaluate existing constructors and `match` for instruction and addressing
  mode decode.
- Decide whether ABI-stable record layout is needed for host-facing values.
- Decide whether packed layout is needed at the host boundary.
- Decide whether enum/tagged-union-like decode shapes require new syntax or
  can remain constructor-based.
- Define inspection rendering for any added layout or decode forms.
- Add structured diagnostics for non-ABI-safe layout, unsupported packed forms,
  and non-exhaustive decode shapes if applicable.
- Add CPU-state and opcode-decode examples.

Exit criteria:

- A small CPU-state record and instruction decoder can be expressed clearly in
  Tacit.
- Any ABI-stable layout surface is typed and explicit.
- No Tacit-Full refinement, capability, handler, or row-polymorphism machinery
  is introduced.
- Performance-sensitive lowering choices are recorded for Phase 8 rather than
  pulled into Phase 6.

Outcome:

- Stage 8 adds no new syntax or canonical node. Existing structural records,
  fixed-width integers, bit primitives, typed vectors, and `match` are
  sufficient for the Phase 6 emulator-skeleton decode target.
- CPU and device state use structural records with fixed-width integer fields,
  `Bool` flags, nested status records, and typed-vector handles where mutable
  storage is needed.
- Instruction and addressing-mode decode use explicit fixed-width tag fields
  in ordinary records, with `pat-int` match arms and wildcard fallback for
  illegal or unknown opcodes.
- User-defined constructor/ADT syntax is deferred. Existing constructors are
  not promoted into a typed decode surface beyond `True`/`False`.
- ABI-stable record layout and packed layout are deferred to Stage 10. During
  Stage 8, records are language-level structural products and byte packing
  remains `u8vec` plus typed byte-bus helper work.
- No new structured diagnostics are added because no new accepted forms are
  added. `non-abi-safe-layout`, `unsupported-packed-layout`, and static
  exhaustiveness diagnostics remain tied to future ABI, packed-layout, or ADT
  surfaces.
- Durable examples live under `examples/phase-6/data-layout/`, with focused
  typecheck and codegen tests covering CPU-state records and opcode-decode
  records.

## Stage 9: Source-Level Stdlib Foundations

**Status:** Design accepted 2026-05-16 by
[ADR 0087](../decisions/0087-phase-6-source-level-stdlib-foundations.md);
implementation complete.

**Purpose:** Start moving library logic out of compiler-recognized primitives
once modules and packages make source libraries viable.

Work items:

- Write the source-stdlib ADR. Done:
  [ADR 0087](../decisions/0087-phase-6-source-level-stdlib-foundations.md).
- Define the source-level stdlib package structure.
- Decide the initial prelude/import behavior, if any.
- Move or wrap byte-order helpers in source-level packages.
- Move or wrap typed-array helpers in source-level packages.
- Add string and collection helpers where the current primitive surface is
  excessive.
- Add file I/O helper wrappers around existing curated primitives.
- Define host-backed capability wrapper conventions for HTTP/network-like
  operations without making networking a built-in primitive.
- Preserve structured type/effect signatures for every public stdlib export.
- Add tests that use stdlib packages through ordinary package imports.

Design outcome:

- Source-level stdlib packages are ordinary hash-pinned packages under
  `stdlib/tacit/`, not compiler-magic packages.
- Stage 9 adds no implicit prelude or name-based `std` resolver; stdlib use
  remains explicit through ordinary manifests, lockfiles, imports, and hashes.
- Initial packages are `tacit.core`, `tacit.bytes`, `tacit.array`,
  `tacit.text`, `tacit.collections`, `tacit.io`, and a convention-only
  `tacit.host` namespace for future Stage 10 capability wrappers.
- The first migration set source-defines simple ASCII/text predicates and
  range-table accessors where possible, while wrapping byte-order,
  typed-vector, collection, and stream I/O primitives as compatibility-safe
  source exports.
- Low-level fixed-width arithmetic, typed-vector memory operations,
  file-descriptor host calls, and higher-order combinators remain
  compiler-recognized in Stage 9 where they still need codegen or checker
  cooperation.
- Networking and HTTP remain host-provided capability patterns for Stage 10,
  not built-in primitives.

Implementation outcome:

- Ordinary source packages now live under `stdlib/tacit/` for `core`,
  `bytes`, `array`, `text`, `collections`, and `io`.
- `stdlib/tacit/text` source-defines ASCII classification/case helpers and
  keeps a package-local helper to exercise visibility boundaries.
- Byte-order, typed-array, collection, UTF-8, and stream I/O wrappers expose
  explicit public signatures while the lower-level primitives remain
  compiler-recognized compatibility shims.
- The `stdlib/tacit/host/` namespace is documentation-only in Stage 9 and
  adds no networking, HTTP, arbitrary FFI, or dynamic plugin loading.
- Integration tests load the stdlib packages as ordinary packages, consume
  `tacit.text` through a path dependency and exact definition-hash import,
  and verify package-local stdlib helpers are not externally importable.

Exit criteria:

- At least one source-level stdlib package is consumed like any other package.
- Public stdlib exports have explicit type/effect signatures.
- The implementation removes or wraps at least one compiler-recognized helper
  without regressing existing examples.
- Networking remains a host-provided capability pattern, not arbitrary FFI.

## Stage 10: Host-Interface ABI

**Status:** Complete 2026-05-16. Design accepted by
[ADR 0088](../decisions/0088-phase-6-host-interface-abi.md);
implementation verified in canonical, view, typechecker, interface, CLI, and
LLVM-feature regression tests.

**Purpose:** Specify and implement the constrained embedding ABI once modules,
packages, systems primitives, and typed memory are stable enough to define the
boundary honestly.

Work items:

- Write the host-interface ABI ADR. Done:
  [ADR 0088](../decisions/0088-phase-6-host-interface-abi.md).
- Define the ABI-expressible Tacit type subset. Done.
- Decide whether captured closures, effect-polymorphic functions, and mutable
  handles can cross the host boundary or are rejected. Done.
- Specify stable C ABI naming, calling convention, and symbol generation. Done.
- Generate machine-readable interface metadata from canonical artifacts. Done.
- Generate C headers from interface metadata. Done.
- Generate Rust host bindings from interface metadata. Done.
- Define host-provided imports with explicit type/effect signatures. Done.
- Define ownership and lifetime rules for values crossing the boundary. Done.
- Define allocator-boundary rules. Done.
- Define result/error ABI. Done.
- Define capability/effect declarations for host-backed operations. Done.
- Decide compile targets for Phase 6: LLVM-native linkable artifacts only, or
  LLVM-native plus an initial WASM target. WASM remains optional unless this
  ADR explicitly accepts it. Done.
- Add diagnostics for ABI-inexpressible exports/imports. Done.
- Add tests for generated headers, generated Rust bindings, and host import
  satisfaction. Done.

Design outcome:

- Host-provided imports are canonical `host-imp` declarations inside unit
  import tables. Their identity is the BLAKE3 hash of the canonical
  capability, operation, and signature declaration; Tacit bodies refer to them
  through ordinary `ref` hashes.
- Interface metadata is generated deterministically at
  `.tacit/cache/packages/<package-hash>/interface.json` from public package
  exports and reachable host imports. Manifest-only host export selection is
  rejected for Stage 10 because manifest bytes are not package identity.
- The ABI-expressible value subset is monomorphic fixed-width scalars,
  `Bool`, legacy `Int`, unit-like empty records, ABI-records whose fields are
  expressible, and typed vector handles only as host-owned borrowed function
  parameters.
- Function values, captured closure values, type/effect-polymorphic
  functions, legacy `Buf`/`I64Vec`, typed vector results, vector fields,
  strings, owned arrays, opaque pointers, and raw addresses are rejected at
  the host boundary.
- C ABI symbols are hash-based. Generated exports and host callbacks use a
  package-specific context and callback table rather than arbitrary external
  linker symbols.
- Scalars and records cross by value. Borrowed vectors are valid only for the
  dynamic extent of a call. No owned heap value crosses the boundary, and
  Stage 10 exposes no allocator hooks.
- Source-level failures remain ordinary Tacit return values. ABI status covers
  invalid host arguments, missing host callbacks, and host callback errors;
  existing runtime traps remain non-recoverable process aborts in Phase 6.
- Host import effects use the existing Tacit-Lite effect atoms, and every host
  import includes `IO`. No new user-defined effects, capability tokens,
  handlers, or row-polymorphic effects are added.
- Phase 6 commits to LLVM-native linkable artifacts only. WASM remains
  deferred and must be rejected as an unsupported host target during Stage 10
  implementation.

Implementation outcome:

- Canonical `host-imp` declarations parse, emit, hash, and sort inside unit
  import tables, with authoring and inspection rendering for host imports.
- Unit checking resolves `ref` nodes to declared host imports by
  `host_import_hash`, checks declared host import signatures, and rejects host
  imports whose flattened function effects omit `IO`.
- `tacit interface` checks a package and writes deterministic
  `.tacit/cache/packages/<package-hash>/interface.json`, plus generated C
  headers and Rust bindings under `.tacit/derived/package-<package-hash>/host/`.
- Interface generation rejects unsupported WASM targets, non-function public
  exports, function-value boundary types, effect variables, legacy handles, and
  invalid typed-vector positions with the Stage 10 diagnostic kinds.
- Regression coverage includes canonical host import round trips, authoring and
  inspection host import rendering, host import type/effect satisfaction,
  interface metadata/header/Rust binding generation, CLI output paths, and
  unsupported WASM rejection.

Exit criteria:

- A Tacit package can compile to a linkable artifact plus interface metadata.
- Generated C headers describe exported Tacit functions.
- Generated Rust bindings can call exported Tacit functions.
- Host-provided imports are type/effect checked against Tacit declarations.
- ABI-inexpressible values are rejected with structured diagnostics.
- No arbitrary C library can be named from Tacit source.

## Stage 11: Embedding Demo And Systems Example

**Status:** Planned

**Purpose:** Prove the Phase 6 end-to-end story without expanding into a full
emulator, GUI framework, or Phase 7 tooling project.

Work items:

- Build a small C or Rust host that links a Tacit package.
- Have the host call exported Tacit logic through generated bindings.
- Have the host satisfy at least one import with explicit effects.
- Include a systems-shaped Tacit package with CPU-state, memory-bus, and
  instruction-decode components.
- Include package-level tests for the systems-shaped package.
- Include structured test output in CI.
- Document how the demo uses the host model without exposing arbitrary FFI.
- Record known performance limitations for Phase 8.

Exit criteria:

- The demo builds and runs in CI.
- The Tacit package is checked, compiled, tested, and linked through the
  constrained host ABI.
- The host owns platform-like capabilities; Tacit owns the typed logic kernel.
- The systems example demonstrates emulator-style expressiveness without being
  a full emulator.

## Stage 12: Primer, Evaluation, And Freeze

**Status:** Planned

**Purpose:** Close Phase 6 with prompt-facing documentation, regression
evidence, and a freeze ADR.

Work items:

- Update the Tacit-Lite primer for modules, packages, tests, systems
  primitives, typed memory, source-level stdlib usage, and host imports/exports.
- Keep primer text language-facing: no repository paths, phase process notes,
  CI instructions, corpus logistics, or recipes tailored to known corpus tasks.
- Re-baseline the `o200k_base` token count after primer edits.
- Add durable Phase 6 examples under `examples/phase-6/`.
- Run project/package/unit/host-interface regression tests.
- Re-run relevant open-corpus or open-benchmark checks only if useful for
  fluency and maintenance comparison.
- Do not use sealed-corpus feedback.
- Write the Phase 6 freeze ADR.

Exit criteria:

- Primer changes are accepted and token count is recorded.
- Phase 6 examples are durable and covered by tests.
- A multi-module Tacit package can be checked, compiled, tested, dependency
  resolved by hash, and consumed by a C or Rust host.
- Host-provided capabilities are visible through explicit type/effect
  signatures.
- The systems primitive surface is sufficient for an emulator-style CPU core,
  memory bus, and instruction decoder.
- Phase 6 freeze ADR is accepted.

## Phase 6 Exit Criteria

Phase 6 is complete when all of the following are true:

- A multi-module Tacit package can be checked as a whole graph.
- A multi-module Tacit package can be compiled deterministically.
- Package dependencies resolve by hash through a manifest and lockfile.
- A local hash-indexed dependency cache verifies artifacts before use.
- Package-level tests run and emit structured results.
- Fixed-width integers, bit operations, byte-order helpers, and typed mutable
  memory are usable in compiled Tacit code.
- Data-layout and decode examples are expressive enough for an emulator-style
  CPU core, memory bus, and instruction decoder skeleton.
- At least one source-level stdlib package is consumed through ordinary package
  imports.
- A C or Rust host can consume a Tacit package through the constrained
  embedding ABI.
- Host-provided imports are type/effect checked.
- Generated interface metadata, C headers, and Rust bindings are covered by
  tests.
- No arbitrary FFI, untyped pointer escape hatch, dynamic plugin mechanism, or
  semantic-version dependency solver is introduced.
- No work relies on `corpus/sealed/`.

## Phase 7 Handoff

Phase 7 should inherit real module, package, test, systems, and host-interface
boundaries to inspect. Debugging, structural diff, blame, merge, IDE, and broad
package-tooling work should be designed against those real boundaries rather
than simulated single-file programs.

Any Phase 6 evidence that points to a narrow missing tool should be recorded in
the Phase 6 freeze ADR, but should not expand Phase 6 unless it blocks the exit
criteria above.
