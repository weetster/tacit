# 0078 - Phase 5 decision: proceed to Phase 6

**Status:** Accepted
**Date:** 2026-05-09
**Phase:** 5, Stage 4
**Closes:** [phase-5-plan.md Stage 4](../plans/phase-5-plan.md)

## Context

Phase 5 was defined as a short evidence gate before modules, packages, systems
primitives, and the host-interface ABI ([ADR 0076](0076-phase-5-short-gate.md)).
The required artifacts are now present:

- the open benchmark spec under
  [plans/phase-5-benchmark/](../plans/phase-5-benchmark/)
- the blind Stage 2 baseline run
  `019e0f6b-7b6f-7832-98a0-a5c72e7545be` under
  [plans/phase-5-results/](../plans/phase-5-results/)
- the metric ADR [0077](0077-phase-5-metrics.md)

The Stage 4 decision must choose exactly one action:

1. Proceed to Phase 6.
2. Build one narrow proven tool before Phase 6.
3. Revise the benchmark and rerun the gate.
4. Pause engineering and publish the Phase 0-4 artifact.

Per ADR 0077, the gates are artifact integrity, final program recovery, and
final explanation correctness on the authoring-facing `main.taca` surface. The
other recorded quantities are descriptive and exist to show whether a narrowly
scoped blocker is visible.

## Decision

**Proceed to Phase 6. Do not build a pre-Phase-6 maintenance tool from the
Phase 5 baseline.**

The recorded baseline passes all three Phase 5 gates:

1. **Artifact integrity gate:** pass.
   The run preserved prompt, raw response, starting artifacts, diagnostics,
   inspection text, and per-turn results for every task.
2. **Program recovery gate:** pass.
   Final program pass rate was `6/6`.
3. **Explanation gate:** pass.
   Final explanation pass rate was `2/2`.

The descriptive metrics do not expose a blocker strong enough to justify a
Phase 5 tool spike:

- One-shot pass rate was `5/8`; final pass rate was `8/8`.
- The repair loop recovered all turn-0 failures: `3/3`.
- Compile/type recovery was `1/1`.
- Behavioral recovery was `5/5`.
- Explanation correctness improved from `0/2` one-shot to `2/2` final.
- Total model calls were `12`, or `1.50` per task.
- Human review cost was `5 low`, `3 medium`, `0 high`.
- Failure stages were limited to `run: 2` and `explanation: 2`; there were no
  API, extraction, typecheck-on-generated-output, or compile-on-generated-output
  failures.

The token-cost split also does not argue for an immediate tool investment:

- Language primer tokens: `265,884`
- Workflow prompt tokens: `0`
- Tool-context tokens: `1,001`
- Generated output tokens: `1,562`

The dominant recurring cost is the Phase 4 language primer, not missing
maintenance instrumentation. The only repaired program task (`r4-map-destination`)
did not reveal a missing diagnostic surface; it converged after behavioral
feedback within the allowed budget. The two explanation misses were specificity
misses (`left`/`right` and `7`), not evidence that inspection or structured
diagnostics were absent or unusable.

Therefore the Phase 5 baseline supports a narrow conclusion: the existing
single-program maintenance/debugging surface is good enough to stop blocking
Phase 6. This is not a claim that Phase 7 tooling is unnecessary; it is a
claim that Phase 6 should not be delayed for it.

## Alternatives considered

### Build one narrow tool before Phase 6

Rejected. The baseline did not show an unrecovered failure mode or a repeated
ambiguous failure that points to one missing tool as a necessary predecessor.
The repaired tasks were recoverable with the existing surface, and review cost
never rose to `high`.

### Revise the benchmark and rerun the gate

Rejected. The benchmark already answered the Phase 5 question cleanly enough
for sequencing. It covered compile/type repair, behavioral repair, edit work,
and explanation tasks, and the accepted metric now interprets those results
without after-the-fact changes.

### Pause engineering and publish the Phase 0-4 artifact

Rejected. That outcome would fit a failed or indecisive baseline. The actual
baseline is decisive enough to proceed: all gates passed, all turn-0 failures
recovered, and no concrete pre-Phase-6 tool blocker emerged.

## Consequences

- Phase 5 is complete.
- Phase 6 may begin.
- Phase 6 work should focus on modules, packages, systems primitives, and the
  constrained host-interface ABI, not on broad debugging/tooling expansion.
- Full debugger, diff, blame, merge, IDE, and package-tooling work remains
  Phase 7 unless a later bounded ADR reopens one narrow tool with new
  evidence.
- The Phase 5 baseline remains a useful reference point for later comparisons,
  especially after Phase 6 introduces multi-file and package boundaries that
  future tooling will need to inspect.
- No sealed-corpus contents, paths, or metadata were used to reach this
  decision.
