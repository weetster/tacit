# Phase 3 Repair-Loop Experiment

**Status:** Open-scope run completed; sealed repair policy still deferred
**Date:** 2026-05-02

## Summary

Run a new open-scope experiment that keeps the existing primer and task
format, but allows the model to repair a failed solution for at most two
additional turns. This is not a replacement for the Phase 3 one-shot gate.
It measures whether Tacit becomes viable in an agentic write-check-repair
workflow.

Outcome: the 12-task canary (`019de6df-becb-7194-90e8-b8ec44d71b84`) improved
from 3/12 to 10/12 final passes. The full open run
(`019de6ef-e75e-70d8-aa52-e98c4c577f7d`) improved from 30/47 to 40/47 final
passes. The full run cleared the final-pass threshold but missed the invalid
recovery threshold by one task (5/11 recovered, below the required half), so it
is useful evidence for an agentic direction but not a replacement for the
one-shot Phase 3 gate. See
[ADR 0060](../decisions/0060-p3-repair-loop-outcome.md).

## Motivation

The primary one-shot Sonnet runs did not clear the >70% task-pass gate. The
best open-only result was 29/47 tasks, and the latest run regressed to 24/47.
The failure shape suggests a repair loop is worth measuring:

- Some failures are invalid outputs: extraction, typecheck, compile, or name
  errors.
- Some failures are valid executable Tacit with behavioral bugs.
- The compiler and test harness already produce structured feedback that a
  model can use in a second pass.

The new question is:

> Given the primer, the task statement, the model's previous Tacit attempt, and
> machine feedback, can the model repair the program within two turns?

That is a different claim from the one-shot primer-only claim. Both metrics
should be reported separately.

## Primer Baseline

Use the primer version from run `019de600-a048-7beb-85d5-648bccd6fea3`.
That run tied for the best full-task pass rate at 29/47 and had the better
secondary profile among the tied runs:

| Run ID | Commit | Primer Hash | Primer Tokens | Full Passes | Compile Pass Rate | Per-Test Passes |
|---|---|---|---:|---:|---:|---:|
| `019de465-4863-7a63-acf1-8040597b2f66` | `0a9424b` | `cdb97776528dc34b6ff053ad177f57478902b0ff9f22a519d95fa265e56d48d7` | 13,762 | 29/47 | 72.3% | 212/234 |
| `019de600-a048-7beb-85d5-648bccd6fea3` | `1cb0f91` | `4879fd9a06777f8f86e03ad73a22fe180f75d1b5b8bccf1b69936c65fe6110af` | 15,533 | 29/47 | 78.7% | 216/246 |

The second run is the preferred repair-loop baseline because repairs benefit
from a primer that already yields more compileable and partially correct
programs.

## Scope

- Provider/model: Anthropic `claude-sonnet-4-6` first.
- Corpus scope: open tasks only.
- Task count: start with a fixed canary subset before any full 47-task run.
- Sampling: temperature `0`, max output `8192`, same as primary Phase 3.
- Repair budget: initial generation plus at most two repair turns.
- Sealed tasks: out of scope until the open canary and open full run justify
  the cost and methodology.

## Non-Goals

- Do not reinterpret the one-shot Phase 3 gate.
- Do not count a repair-loop pass as a one-shot pass.
- Do not tune prompts from sealed results.
- Do not expand the standard library as part of this experiment.
- Do not use Python or Tacit reference solutions as translation hints.

## Protocol

Each task gets up to three model calls:

1. **Turn 0: generate.**
   Send the primer as the system message and the task statement as the user
   message, exactly as the current primary harness does.
2. **Grade turn 0.**
   Extract one `tacit` fenced block, synthesize sidecar metadata, run
   typecheck, compile, and tests.
3. **Turn 1: repair if needed.**
   If turn 0 fails, send a new user message containing the task statement, the
   previous generated Tacit, and feedback from the first failed stage.
4. **Grade turn 1.**
   Run the same extraction, typecheck, compile, and test pipeline.
5. **Turn 2: repair if needed.**
   If turn 1 fails, repeat the repair prompt once more using the latest
   generated Tacit and latest feedback.
6. **Final score.**
   The task is a repair-loop pass if any turn produces a program that passes
   all tests.

The run records both first-pass and final-pass metrics. A task that passes on
turn 0 should not be sent through repair.

## Feedback Payload

The repair prompt should include only information that would be available in a
real local development loop:

- The original task statement.
- The previous Tacit program.
- The failed stage: `extract`, `typecheck`, `compile`, or `test`.
- The structured diagnostic envelope for extraction, typecheck, or compile
  failures.
- For open-task test failures, a compact summary of failing cases.

For open tasks, include at most the first two failing test details:

- test name
- stdin
- expected stdout
- actual stdout or failure reason

Do not include all tests by default. The repair loop should provide enough
feedback to debug, not turn the prompt into a copyable test corpus.

For any future sealed run, test feedback must remain redacted or summarized
without exposing concrete inputs and expected outputs. The first repair-loop
experiment is open-only to avoid settling that policy prematurely.

## Repair Prompt Shape

Use a stable repair suffix so runs are comparable:

````text
The previous Tacit-Lite program failed.

Task:
<task statement>

Previous program:
```tacit
<previous generated program>
```

Failure stage: <extract|typecheck|compile|test>

Feedback:
<diagnostic or compact failing-test summary>

Return a corrected solution as a single Tacit-Lite program in one fenced
block: ```tacit ... ```. Do not include the sidecar. Do not include
explanatory prose.
````

If extraction failed because there was no single fenced block, include the raw
model text only if it is short enough to be useful. Otherwise report the
extraction diagnostic and ask for a single fenced block.

## Metrics

Per task, record:

- `turns_used`: `0`, `1`, or `2` for the passing turn; `null` on failure.
- `first_pass_compile_pass`
- `first_pass_typecheck_pass`
- `first_pass_tests_pass`
- `final_compile_pass`
- `final_typecheck_pass`
- `final_tests_pass`
- `repair_success`: failed turn 0 but passed by turn 2.
- `failure_stage_by_turn`
- `generation_tokens_by_turn`
- `diagnostics_by_turn`

Aggregate metrics:

- one-shot task pass rate
- final task pass rate after two repair turns
- repair recovery rate among initially failed tasks
- compile/typecheck recovery rate
- behavioral recovery rate
- average model calls per task
- total generation tokens across all turns
- total API calls

Token economics are reported but not gating for this experiment.

## Canary

Before a full open run, use a fixed 12-task canary drawn from known open
failures of `019de600-a048-7beb-85d5-648bccd6fea3`:

| Task | Best-run failure class |
|---|---|
| `algorithms/033-two-sum` | typecheck |
| `algorithms/035-bubble-sort` | compile |
| `algorithms/037-merge-sort` | extraction |
| `algorithms/044-count-islands` | behavioral |
| `algorithms/049-matrix-multiply` | compile |
| `collections/021-unique-in-order` | behavioral |
| `collections/026-group-counts` | behavioral |
| `collections/027-rotate-left` | typecheck |
| `io/054-grep-substring` | unbound name |
| `io/055-sort-lines` | behavioral |
| `io/056-unique-lines` | behavioral |
| `strings/015-title-case` | typecheck |

This canary is intentionally mixed. It tests whether repair turns recover
syntax/shape failures and whether test feedback improves semantic failures.

## Success Criteria

Proceed from canary to a full open run only if the canary shows a material
gain:

- final pass rate improves by at least 4 tasks out of 12 over turn 0, and
- at least two invalid-output failures are repaired, and
- at least two behavioral failures are repaired, and
- average model calls stay below 2.5 per task.

For a full open run, the experiment is promising if:

- final pass rate reaches at least 33/47, matching the original >70% threshold
  on open tasks, and
- at least half of initially invalid Tacit failures are repaired, and
- at least one third of initially behavioral failures are repaired.

These thresholds do not declare Phase 3 passed. They justify treating
agentic repair as the next product/evaluation direction.

## Harness Changes

Implement this as an explicit mode rather than changing primary evaluation:

```bash
uv run corpus-eval --model claude-sonnet-4-6 --repair-turns 2 --tasks ...
```

Rules:

- Default `--repair-turns` is `0`, preserving current behavior.
- `--repair-turns 2` enables the repair protocol above.
- Metrics include repair fields only when repair turns are enabled.
- Existing primary metrics remain valid and comparable.
- Failure artifacts store `turn-0`, `turn-1`, and `turn-2` subdirectories.
- `--dry-run` should exercise the shape without API calls, likely by treating
  the first synthetic output as final success.

## Cost Control

- Start with the 12-task canary.
- Use the restored best-run primer.
- Keep temperature `0`.
- Stop repairing a task immediately after it passes.
- Do not run sealed tasks until the open full run is worth reporting.
- Do not run Haiku or cross-family models until Sonnet shows a useful repair
  delta.

Worst-case canary cost is 36 model calls. A full open run is at most 141 calls
but should be lower if many tasks pass before turn 2.

## Interpretation

Possible outcomes:

1. **Large repair gain.** Tacit may be better evaluated as an agentic language
   with compiler-in-the-loop feedback than as a one-shot generation target.
2. **Only invalid-output failures recover.** The primer teaches enough syntax
   for repair, but algorithmic/semantic performance remains the bottleneck.
3. **Only behavioral failures recover.** The compiler surface is still too hard
   for the model, but tests help once source is valid.
4. **Small or no gain.** More paid evals should stop; either fine-tuning or
   language/stdlib redesign is the next meaningful change.

This experiment should be reported next to the one-shot result, not merged
into it.
