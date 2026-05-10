# Phase 5 Results Note

This directory records the Phase 5 Stage 2 current-tool baseline artifacts for
the open maintenance/debug benchmark. The run used the Phase 5 adapter on the
authoring-facing `main.taca` inputs and did not access `corpus/sealed/`.

## Run

- Run ID: `019e0f6b-7b6f-7832-98a0-a5c72e7545be`
- Completed at: `2026-05-10T01:06:49Z`
- Model: `claude-sonnet-4-6`
- Provider: `anthropic`
- Manifest: `plans/phase-5-benchmark/manifest.json`
- Repair turns: `2`
- Primer tokens: `22,157`
- Output file: `019e0f6b-7b6f-7832-98a0-a5c72e7545be/run.json`

The per-task raw prompts, raw responses, diagnostics, inspection output, and
generated `generated.taca` files live under
`plans/phase-5-results/019e0f6b-7b6f-7832-98a0-a5c72e7545be/`.

## Summary

- Task count: `8`
- Final passes after repair: `8/8`
- One-shot passes: `5/8`
- Recovered by repair loop: `3/3`
- Total model calls: `12`
- Average model calls per task: `1.50`
- Total generation tokens: `1,562`

Task-level outcome:

| Task | Class | Starting state | One-shot | Final |
| --- | --- | --- | ---: | ---: |
| `r1-record-total` | Repair | Compiled, wrong exit (`12`) | Pass | Pass |
| `r2-closure-offset` | Repair | Compiled, wrong exit (`64`) | Pass | Pass |
| `r3-record-field` | Repair | Type/compile failure (`missing-field`) | Pass | Pass |
| `r4-map-destination` | Repair | Compiled, wrong exit (`15`) | Fail | Pass |
| `e1-record-bonus` | Edit | Compiled, wrong exit (`33`) | Pass | Pass |
| `e2-closure-scale` | Edit | Compiled, wrong exit (`26`) | Pass | Pass |
| `x1-missing-record-field` | Explanation | Type/compile failure (`missing-field`) | Fail | Pass |
| `x2-non-function-map` | Explanation | Type/compile failure (`callback-type-mismatch`) | Fail | Pass |

## Observed Failures

This note is descriptive only; Phase 5 Stage 3 still defines the metric ADR
before these results become decision evidence.

- `r4-map-destination` needed both repair turns. The first two generations
  still returned the wrong exit (`36`, then `15`) before the third generation
  corrected the final read path and reached exit `18`.
- `x1-missing-record-field` failed one-shot because the explanation stayed too
  generic and omitted the benchmark-required field names `left` and `right`.
- `x2-non-function-map` failed one-shot because the explanation described the
  general `@map` callback contract but omitted the concrete non-function value
  `7` present in the callback slot.

## Interpretation Boundary

Stage 2 is now complete because the repository has a blind rerun record with
enough prompts, diagnostics, outputs, inspection text, and task results to
support Stage 3 metric design. No sealed-corpus contents, paths, or metadata
were read or recorded while producing this baseline.
