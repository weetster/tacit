# 0079 - Phase 6 scope and stage plan

**Status:** Accepted
**Date:** 2026-05-10
**Phase:** 6, Stage 0
**Closes:** [phase-6-plan.md Stage 0](../plans/phase-6-plan.md)

## Context

Phase 5 is complete. [ADR 0078](0078-phase-5-decision.md) accepted the
decision to proceed to Phase 6 without building a pre-Phase-6 maintenance
tool. The handoff is deliberately narrow: Phase 6 should focus on modules,
packages, systems primitives, unit testing, source-library foundations,
dependency caching, and the constrained host-interface ABI.

The master plan already excludes arbitrary FFI, untyped pointer escapes,
semantic-version dependency solving, public registry operation, a full video
game emulator, and broad debugging/IDE tooling from Phase 6. The project needs
a binding Phase 6 scope artifact before implementation begins so later stages
do not relitigate those boundaries while designing module, package, systems,
or host-interface details.

Phase 6 planning must also preserve the sealed-corpus boundary. No Phase 6
development work may read, list, search, or otherwise access
`corpus/sealed/`.

## Decision

**Accept [plans/phase-6-plan.md](../plans/phase-6-plan.md) as the active Phase
6 scope and stage plan.**

The accepted scope is:

1. Module semantics for imports, exports, visibility, explicit type/effect
   signatures, content-hash identity, sidecar aliases, mutual-recursion group
   boundaries, and imported-hash type/effect checking.
2. Multi-file project support and whole-project graph commands that preserve
   the rule that file layout has no semantic weight.
3. A local package model, manifest, lockfile, dependency cache, and object
   store based on dependency hashes rather than semantic-version ranges.
4. Package-level unit testing with structured results.
5. Fixed-width integer, bit-operation, byte-order, typed mutable-memory, and
   data-layout/decode support sufficient for emulator-style systems code.
6. Source-level stdlib foundations now that modules and packages make source
   libraries viable.
7. A constrained host-interface ABI for C/Rust embedding, including generated
   interface metadata, C headers, Rust host bindings, host-provided imports,
   ownership/lifetime rules, result/error ABI, allocator-boundary rules, and
   capability/effect declarations.
8. A small embedding demo proving the "Tacit logic kernel inside a
   conventional host" model.
9. Primer updates, durable Phase 6 examples, regression evidence, and a Phase
   6 freeze ADR.

The accepted non-goals are:

- No arbitrary `extern "C"` declarations from Tacit source.
- No direct Tacit bindings to arbitrary ecosystem libraries.
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

The accepted Stage 0 artifact also records Q-P6-1 through Q-P6-15 with
resolution stages and defines the required ADR sequence. Implementation work
for later stages must not proceed ahead of the ADRs that settle their design
dependencies.

## Alternatives considered

### Start implementation from the master plan only

Rejected. The master plan gives the right top-level direction, but Phase 6 is
large enough that implementation needs a binding stage plan, open-question
table, and ADR sequence before code changes begin.

### Split Phase 6 before Stage 0

Rejected for now. The scope is broad, but the work is ordered around one
coherent dependency chain: module semantics, project graph, packages, tests,
systems primitives, source libraries, host ABI, and demo. Splitting before
Stage 1 would create artificial phase boundaries before the module/package
design is known.

### Pull Phase 7 tooling forward

Rejected. ADR 0078 found no pre-Phase-6 tool blocker. Phase 7 should consume
real module, package, test, systems, and host-interface boundaries produced by
Phase 6 rather than building broad tools against simulated single-file
programs.

### Permit general FFI as the host-interface shortcut

Rejected. This would violate [ADR 0022](0022-pure-kernel-host-model.md): Tacit
is a pure computational kernel, and arbitrary ecosystem impurity belongs in
the host. Phase 6's embedding ABI is the intended answer, not an escape hatch.

## Consequences

- Stage 0 is complete.
- Stage 1 may begin with the module imports/exports/visibility ADR.
- Phase 6 implementation work is blocked on the relevant design ADRs, not on
  additional scope negotiation.
- The sealed-corpus restriction remains active for all Phase 6 work.
- Broad debugger, diff, blame, merge, Git driver, IDE, and package-tooling work
  remains Phase 7 unless a later bounded ADR reopens one narrow tool with new
  evidence.
- The Phase 6 freeze ADR must verify the accepted exit criteria: a
  multi-module Tacit package can be checked, compiled, tested, resolved by
  hash, and consumed by a C or Rust host through the constrained embedding ABI.

## Related decisions

- [ADR 0022](0022-pure-kernel-host-model.md) - pure computational kernel and
  host-owned impurity.
- [ADR 0078](0078-phase-5-decision.md) - proceed to Phase 6 without a
  pre-Phase-6 maintenance tool.
- [plans/phase-6-plan.md](../plans/phase-6-plan.md) - accepted Phase 6 stage
  plan.
