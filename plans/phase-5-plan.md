# Phase 5 Plan

**Status:** Active, scoped by
[ADR 0076](../decisions/0076-phase-5-short-gate.md)
**Scope:** Bounded maintenance/debugging validation gate before Phase 6

## Context

Phase 4 is frozen by [ADR 0075](../decisions/0075-phase-4-frozen.md). It
delivered the current maintenance/debugging substrate: structured diagnostics,
`tacit view --as inspection --types --effects`, a repair-loop harness, durable
examples, and strong open-corpus repair results.

The master plan puts maintenance/debugging validation before modules and
packages, but the dependency is strategic rather than mechanical. Phase 6 can
be implemented without a Phase 5 tool surface; what Phase 5 must decide is
whether the project should proceed to Phase 6 now, first build one proven
maintenance tool, revise the benchmark, or pause and publish the Phase 0-4
artifact.

Therefore Phase 5 is a short evidence gate. It is not a full debugger, diff,
blame, IDE, or package-tooling phase.

No Phase 5 work may read, list, search, or otherwise access
`corpus/sealed/`. Sealed grading, if ever requested, is an operator-triggered
evaluation action and must not provide development feedback.

## Goal

Produce enough maintenance/debugging evidence to make a sequencing decision
without delaying Phase 6 behind broad tooling work.

The expected fast path is:

1. Define an open maintenance/debug benchmark.
2. Run a baseline using only Phase 4-era tools.
3. Write the metric ADR before interpreting results.
4. Write a decision ADR choosing the next phase action.

## Non-Goals

- No full `tacit-debug`.
- No structural diff, blame, merge, or Git-driver implementation.
- No module/package/host-interface implementation.
- No new language surface.
- No expansion of the Tacit-Lite authoring primer for workflow instructions.
- No sealed-corpus inspection or sealed-development feedback.

## Benchmark Shape

The benchmark is an open, small, maintenance-oriented task set. It should be
large enough to test editing and debugging behavior, not just fresh generation,
but small enough to complete quickly.

Task classes:

| Class | Count | Purpose |
| --- | ---: | --- |
| Repair | 4-6 | Fix compile, type/effect, or behavioral failures in existing Tacit programs. |
| Edit | 2-3 | Add a small feature to a working Tacit program. |
| Explanation | 2-3 | Explain a failure using structured diagnostics and inspection output. |

Task sources:

- Prefer hand-authored open programs under a Phase 5-specific benchmark
  directory.
- Open corpus references may be used only if the task is not part of a primary
  generation comparison for the same run.
- `examples/phase-3/` and `examples/phase-4/` may be used as style references,
  but benchmark tasks should not simply duplicate those examples.
- `corpus/sealed/` is forbidden as a source, including paths and metadata.

The benchmark spec must record, per task:

- Starting files supplied to the agent.
- Prompt text supplied to the agent.
- Allowed tool surface.
- Expected pass condition.
- Whether the task grades compile recovery, type/effect recovery, behavioral
  recovery, explanation quality, or some combination.

## Metrics

Phase 5 must not collapse maintenance cost into one density number. Before
interpreting any run, write a metric ADR that separates at least:

- Repair turns.
- Model calls.
- Language-primer context.
- Workflow-primer/runbook context.
- Tool/schema context.
- Generated output.
- Compile recovery.
- Type/effect recovery.
- Behavioral recovery.
- Explanation correctness.
- Human review cost.

The metric ADR may reuse the Phase 3 metrics schema where it fits, but it must
make maintenance-specific quantities explicit.

## Stages

### Stage 0: Scope Lock

**Status:** Complete.

Work items:

- Confirm Phase 5 is a short gate, not a broad tooling phase.
- Confirm Phase 6 remains blocked only on the Phase 5 decision ADR, not on
  debugger/diff/blame implementation.
- Preserve the sealed-corpus boundary.

Exit criteria:

- This plan exists.
- ADR 0076 is accepted.
- The master plan points Phase 6 at the short-gate decision.

### Stage 1: Benchmark Spec

**Status:** Complete 2026-05-09. Deliverable:
[plans/phase-5-benchmark/README.md](phase-5-benchmark/README.md)

Work items:

- Select 8-12 open maintenance/debug tasks using the benchmark shape above.
- Record task prompts, supplied files, allowed tools, and grading expectations.
- Document how to run the benchmark without accessing `corpus/sealed/`.

Exit criteria:

- A benchmark spec exists under `plans/` or an open benchmark directory.
- Every task has a clear pass condition.
- The spec explicitly excludes sealed corpus contents, paths, and metadata.

### Stage 2: Current-Tool Baseline

**Status:** Complete 2026-05-09. Deliverables:
[plans/phase-5-results/README.md](phase-5-results/README.md),
[plans/phase-5-results/019e0f6b-7b6f-7832-98a0-a5c72e7545be/run.json](phase-5-results/019e0f6b-7b6f-7832-98a0-a5c72e7545be/run.json)

Allowed tool surface:

- Structured compiler/type/effect diagnostics.
- `tacit view --as inspection --types --effects`.
- Existing tests and smoke programs.
- Existing repair-loop conventions.
- Normal repository search that excludes `corpus/sealed/`.

Work items:

- Run the benchmark once with the Phase 4-era tool surface.
- Record raw outputs and run artifacts.
- Note observed failures without treating them as success/failure evidence
  until the metric ADR is accepted.

Exit criteria:

- A baseline run record exists.
- The record contains enough diagnostics, prompts, outputs, and test results to
  support later scoring under the metric ADR.

### Stage 3: Metric ADR

**Status:** Pending

Work items:

- Write an ADR defining the Phase 5 maintenance/debug metrics.
- Separate prompt/context/tool/generated/recovery/review costs before reading
  the baseline as a success or failure.
- Decide which metrics are gates and which are descriptive.

Exit criteria:

- The metric ADR is accepted.
- Baseline results can be interpreted without changing the metric after the
  fact.

### Stage 4: Decision ADR

**Status:** Pending

Work items:

- Interpret the benchmark under the accepted metric.
- Choose exactly one next action:
  - Proceed to Phase 6 modules/packages/systems/host-interface work.
  - Build one narrow proven tool before Phase 6.
  - Revise the benchmark and rerun the gate.
  - Pause engineering and publish the Phase 0-4 artifact.

Exit criteria:

- A Phase 5 decision ADR is accepted.
- If the decision is "proceed to Phase 6", Phase 6 may begin.
- If the decision is "build one tool", that tool gets its own bounded ADR and
  must not expand into Phase 7 by accident.

## Phase 6 Handoff

Phase 6 may begin only after Stage 4 accepts a decision ADR that chooses
"proceed to Phase 6" or after any explicitly selected pre-Phase-6 tool spike
is complete.

Full inspection/debugging tooling remains Phase 7. Phase 5 supplies the
evidence and sequencing decision; Phase 6 supplies the module/package/system
boundaries that Phase 7 tooling will later inspect.
