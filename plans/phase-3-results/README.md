# Phase 3 Results Note

Status: regrouping note, written after the seventh open-only Sonnet run.

This note summarizes the paid Phase 3 primary-track runs currently recorded
under this directory. It intentionally covers only the open task scope. No
sealed task contents were read or used while writing this note.

## Scope

- Provider/model: Anthropic `claude-sonnet-4-6`
- Track: primary
- Scope: open tasks only, 47 tasks
- Sampling: temperature `0`, max output `8192`
- Dates: runs completed between `2026-04-30T23:58:46Z` and
  `2026-05-02T00:53:17Z`

No Haiku run and no sealed-scope run is summarized here. The reason is
practical: the open-only Sonnet runs did not clear the correctness gate, so
spending additional credits on sealed or weaker-model runs would not change
the current go/no-go decision.

## Runs

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
33 full task passes. The best recorded open-only result is 29 full task
passes. The latest run regressed to 24.

## Failure Shape

The best recorded run, `019de600-a048-7beb-85d5-648bccd6fea3`, had:

- 29 full task passes
- 10 compile, extraction, or type failures
- 8 behavioral failures after producing runnable Tacit

The latest run, `019de625-16f3-7cc7-9cb9-140b822ce02f`, had:

- 24 full task passes
- 11 compile, extraction, or type failures
- 12 behavioral failures after producing runnable Tacit

This matters for interpretation. The remaining gap is not only missing
library surface. The model still emits invalid Tacit often enough to be a
primary failure mode, and valid generated Tacit still fails ordinary edge
cases often enough to prevent the pass-rate gate from being a near miss.

## Token Gate

All recorded runs fail the current token gate by orders of magnitude. Under
the current harness and ADR 0051, the primer is counted once per task. As the
primer grew from 10,202 to 16,194 tokens, the measured token delta worsened
monotonically.

For the current decision, the token gate is not the useful discriminant: the
open-only correctness result already fails the primary gate. However, the
token data does rule out continuing paid full-corpus runs as a path to a
Phase 3 pass under the current rules.

As additional context from the open reference corpus, hand-authored Tacit
references currently total 20,661 tokens against 4,584 Python tokens. That is
about +351% before primer cost. This is not a model-generation result, but it
shows that the current authoring surface and library surface are not yet
token-competitive with the Python baseline.

## Interpretation

The primer-only core-language experiment has not cleared the bar.

Increasing primer size improved pass rate through the fifth and sixth runs,
then regressed on the seventh. The current data does not support spending more
credits on full open-only reruns without changing the experiment.

Expanding the standard library remains a plausible language-product direction,
but it should be treated as a new hypothesis:

- It may improve task success by letting models compose larger primitives.
- It does not by itself prove that the model has learned to program core
  Tacit from the primer.
- A stdlib-mediated pass should be reported separately from primer-only core
  fluency.

## Decision

Do not run more paid full-corpus Phase 3 evaluations under the current setup.
Do not proceed to sealed or Haiku baseline runs until the open-scope Sonnet
result has a credible path past the correctness gate.

Reasonable next experiments are:

1. Write a short ADR recording that Phase 3 remains unfrozen because the
   primer-only open-scope Sonnet result did not clear the correctness gate.
2. Reframe standard-library expansion as a separate experiment in
   library-mediated Tacit authoring.
3. Define a cheap canary before any new paid full run, for example a fixed
   subset of tasks that previously failed for distinct reasons.
4. Revisit the token metric only through an ADR, not by ignoring the current
   harness output.
