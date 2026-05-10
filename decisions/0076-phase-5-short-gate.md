# 0076 - Phase 5 short gate before Phase 6

**Status:** Accepted
**Date:** 2026-05-09
**Phase:** 5 scope
**Closes:** [tacit-plan.md Phase 5 / Phase 6 sequencing](../plans/tacit-plan.md);
[phase-5-plan.md Stage 0](../plans/phase-5-plan.md)

## Context

[tacit-plan.md](../plans/tacit-plan.md) places maintenance/debugging
validation before modules, packages, systems primitives, and the
host-interface ABI. The dependency is real but strategic: Phase 6 implementation
does not mechanically require a Phase 5 debugger, structural diff, blame, IDE,
or Git integration. It does require a decision that the project should proceed
to modules rather than first investing in one maintenance tool or pausing after
the Phase 0-4 research artifact.

Phase 4 already provides the current single-program inspection substrate:
structured diagnostics, `tacit view --as inspection --types --effects`,
examples, and repair-loop evidence. Holding Phase 6 behind a broad tooling
phase would duplicate work that the plan already assigns to Phase 7, where real
module and package boundaries exist for tools to inspect.

The project still needs the Phase 5 evidence gate. Skipping it entirely would
remove the planned check on the AST-first maintenance/debugging claim.

## Decision

**Phase 5 is a bounded short gate, not a broad tooling implementation phase.**

Phase 5 must complete four artifacts before Phase 6 begins:

1. An open maintenance/debug benchmark spec that does not use or expose
   `corpus/sealed/` contents, paths, or metadata.
2. A baseline run using only the Phase 4-era tool surface: structured
   diagnostics, inspection view with types/effects, tests, and existing
   repair-loop conventions.
3. A metric ADR that separates repair turns, model calls, language-primer
   context, workflow/runbook context, tool/schema context, generated output,
   compile/type/effect recovery, behavioral recovery, explanation correctness,
   and human review cost.
4. A decision ADR choosing exactly one next action: proceed to Phase 6, build
   one narrow proven tool before Phase 6, revise the benchmark, or pause
   engineering and publish the Phase 0-4 artifact.

No Phase 6 implementation work should begin until the Phase 5 decision ADR
chooses "proceed to Phase 6" or an explicitly selected pre-Phase-6 tool spike
has completed.

If a workflow/runbook prompt artifact is introduced, it remains tool-facing and
separate from the Tacit-Lite authoring primer. Full debugger, structural diff,
blame, merge, Git driver, IDE, and registered-view work remains Phase 7 unless
the Phase 5 decision ADR selects one narrow tool as a pre-Phase-6 blocker.

## Alternatives considered

### Do all of Phase 6 before Phase 5

Rejected. This would bypass the planned evidence gate and make the Phase 5
"proceed to Phase 6" decision meaningless.

### Build full debugging tooling before Phase 6

Rejected. The master plan already sequences broad tooling in Phase 7, after
Phase 6 creates real module/package boundaries. Building a full debugger or
diff/blame suite now would delay the module work that later tooling needs to
inspect.

### Keep Phase 5 as an unbounded validation/tooling phase

Rejected. The Phase 4 tool surface is sufficient for a current-tool baseline.
If that baseline identifies a narrow blocker, the Phase 5 decision ADR can
select it explicitly. The default should be evidence first, not tool sprawl.

## Consequences

- Phase 5 can close quickly if the current-tool baseline is good enough to
  proceed.
- Phase 6 remains sequenced after Phase 5, but the required predecessor is a
  decision record, not a large new tool surface.
- Phase 7 retains ownership of the full inspection/debugging tooling suite.
- Metrics must be defined before interpreting the baseline, avoiding
  after-the-fact density or productivity claims.
- The sealed-corpus boundary remains unchanged: no Phase 5 development work may
  read, list, search, or otherwise access `corpus/sealed/`.
