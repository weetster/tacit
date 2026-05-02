# Phase 3 Results Note

Status: regrouping note, updated after the open repair-loop run.

This note summarizes the paid Phase 3 Sonnet runs currently recorded under
this directory. It intentionally covers only open task scope. No sealed task
contents were read or used while writing this note.

## Scope

- Provider/model: Anthropic `claude-sonnet-4-6`
- Track: primary, plus explicit repair-loop mode where noted
- Scope: open tasks only
- Sampling: temperature `0`, max output `8192`
- Dates: runs completed between `2026-04-30T23:58:46Z` and
  `2026-05-02T04:52:24Z`

No Haiku run and no sealed-scope run is summarized here. The reason remains
practical: the one-shot open Sonnet result did not clear the correctness or
token gate, and repair-loop sealed feedback policy is not settled.

## One-Shot Runs

| Run ID | Primer Tokens | Full Task Passes | Task Pass Rate | Compile Pass Rate | Typecheck Pass Rate | Token Delta |
|---|---:|---:|---:|---:|---:|---:|
| `019de0bf-2c96-7d98-a369-20060dab522d` | 10,202 | 3/47 | 6.4% | 10.6% | 10.6% | +10,628.6% |
| `019de19a-25d3-786e-8deb-f2c264eaec55` | 10,571 | 18/47 | 38.3% | 53.2% | 61.7% | +11,221.1% |
| `019de1b6-7f56-744f-9c01-28cbd3419a03` | 11,201 | 22/47 | 46.8% | 66.0% | 80.9% | +11,882.1% |
| `019de1dc-c37e-75ec-8e1f-faf9327ae7ff` | 11,755 | 25/47 | 53.2% | 68.1% | 80.9% | +12,488.0% |
| `019de465-4863-7a63-acf1-8040597b2f66` | 13,762 | 29/47 | 61.7% | 72.3% | 83.0% | +14,491.8% |
| `019de600-a048-7beb-85d5-648bccd6fea3` | 15,533 | 29/47 | 61.7% | 78.7% | 83.0% | +16,291.9% |
| `019de625-16f3-7cc7-9cb9-140b822ce02f` | 16,194 | 24/47 | 51.1% | 76.6% | 85.1% | +16,975.9% |

The Phase 3 primary correctness gate requires Sonnet to exceed 70% task pass
rate on the primary corpus. On 47 open tasks, that would require at least
33 full task passes. The best recorded standalone one-shot run is 29 full task
passes. The repair-loop full run's turn 0 produced 30/47 under the same prompt
shape, still short of the gate.

## Repair-Loop Runs

Repair-loop mode keeps the same primer and task prompt, then gives the model
up to two additional turns with compiler/test feedback. These runs are not
one-shot Phase 3 passes.

| Run ID | Scope | One-Shot Passes | Final Passes | Repair Successes | Invalid Repaired | Behavioral Repaired | Avg Calls | Repair Token Delta |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `019de6df-becb-7194-90e8-b8ec44d71b84` | 12-task canary | 3/12 | 10/12 | 7/9 | 5/7 | 2/2 | 2.42 | +30,450.3% |
| `019de6ef-e75e-70d8-aa52-e98c4c577f7d` | 47-task open | 30/47 | 40/47 | 10/17 | 5/11 | 5/6 | 1.53 | +25,220.5% |

The full open repair-loop run crossed the original open-task correctness
threshold after repair: 40/47 final passes, or 85.1%. It did not satisfy every
repair-loop success criterion from `plans/phase-3-repair-loop-experiment.md`:
invalid-output recovery was 5/11, just below the "at least half" threshold.
The result is still a material signal that compiler-in-the-loop feedback is
useful for Tacit.

The seven remaining full-open failures were:

- `collections/025-partition-eo`
- `collections/026-group-counts`
- `algorithms/035-bubble-sort`
- `algorithms/036-quicksort`
- `algorithms/037-merge-sort`
- `algorithms/049-matrix-multiply`
- `io/055-sort-lines`

These failures are concentrated in sequence transformation, counting,
sorting, matrix, and line-processing code. That is the strongest argument for
a separate library-mediated authoring experiment.

## Token Gate

All recorded runs fail the current token gate by orders of magnitude. Under
ADR 0051, the primer is counted once per model call. In one-shot runs that is
once per task; in repair-loop runs it is once per generation or repair turn.

For the full open repair-loop run:

- First-turn primary aggregate: 754,794 Tacit tokens vs 4,584 Python tokens.
- All-turn repair aggregate: 1,160,690 Tacit tokens vs 4,584 Python tokens.
- Total model output across all turns: 42,314 generation tokens.
- Primer tokens paid across all model calls: 1,118,376 tokens.

The token data rules out additional paid full-corpus reruns as a path to a
Phase 3 pass under the current rules. The open reference corpus also remains
non-competitive before primer cost: hand-authored Tacit references total
20,661 tokens against 4,584 Python tokens, about +351%.

## Interpretation

The primer-only core-language experiment has not cleared the Phase 3 bar.
Increasing primer size improved pass rate through the fifth and sixth
one-shot runs, then regressed on the seventh. More primer-only iteration is
not the right next move.

The repair-loop experiment changes the product direction:

- Tacit is more viable as an agentic write-check-repair workflow than as a
  one-shot generation target.
- Compiler and test feedback repaired both invalid outputs and behavioral
  failures.
- The remaining failures point to missing reusable library operations and
  awkward low-level buffer programming.

Expanding the standard library remains plausible, but it is a new hypothesis:

- It may improve task success by letting models compose larger primitives.
- It may reduce generated tokens if library calls replace repeated low-level
  loops.
- It does not prove primer-only core-language fluency.
- A stdlib-mediated pass must be reported separately from both one-shot
  core-language fluency and repair-loop fluency.

## Decision

Do not run more paid full-corpus Phase 3 evaluations under the current setup.
Do not proceed to sealed, Haiku, or cross-family baselines until the open
Sonnet path has a clearer interpretation.

Next work should be:

1. Record the repair-loop outcome as an ADR/result note.
2. Keep repair-loop accounting explicit in the harness metrics.
3. Define a sealed-safe repair feedback policy before any sealed repair run.
4. Open a stdlib-mediated authoring plan with a cheap canary before any full
   paid rerun.
5. Revisit the token metric only through an ADR, not by ignoring the current
   harness output.
