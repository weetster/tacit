# Phase 4 Results Note

**Phase 4 frozen 2026-05-08 - see
[ADR 0075](../../decisions/0075-phase-4-frozen.md).**

This directory records Phase 4 Stage 8 re-evaluation artifacts. The paid run
was executed with repair-loop mode enabled and written here instead of under
`plans/phase-3-results/`.

## Run

- Run ID: `019e0891-4143-78f6-9146-2c701c408bbb`
- Model: `claude-sonnet-4-6`
- Provider: `anthropic`
- Scope: open corpus
- Repair turns: `2`
- Output files:
  - `019e0891-4143-78f6-9146-2c701c408bbb.run.json`
  - `019e0891-4143-78f6-9146-2c701c408bbb.metrics.json`

## Summary

- Open corpus size: `47` tasks
- One-shot task passes: `38/47`
- Final task passes after repair: `47/47`
- One-shot task pass rate: `80.9%`
- Final task pass rate after repair: `100.0%`
- One-shot compile pass rate: `91.5%`
- One-shot typecheck pass rate: `93.6%`
- Repair-loop recovery rate: `100.0%`
- Average model calls per task: `1.23`
- Total generation tokens: `20,157`
- Primer tokens: `22,157`

Compared with the recorded Phase 3 repair-loop runs, Phase 4 materially
improves fluency:

| Run | One-shot | Final after repair | Repairs | Avg calls |
| --- | ---: | ---: | ---: | ---: |
| Phase 3 core-language `019de6ef-e75e-70d8-aa52-e98c4c577f7d` | `30/47` | `40/47` | `10/17` | `1.53` |
| Phase 3 library-mediated `019df533-fc2a-7511-ad6f-ebdc653878ae` | `32/47` | `46/47` | `14/15` | `1.36` |
| Phase 4 core-language `019e0891-4143-78f6-9146-2c701c408bbb` | `38/47` | `47/47` | `9/9` | `1.23` |

## Density

The token counter reports the open-corpus Rust reference total at `7,064`
tokens and the open Tacit reference total at `42,376` tokens, or about `6.00x`
Rust.

For this paid repair-loop run, the model-output Tacit totals were:

- One-shot: `1,057,570` Tacit tokens, or `149.7x` the Rust reference total
- Repair aggregate: `1,305,263` Tacit tokens, or `184.7x` the Rust reference total

Compared with the recorded Phase 3 open repair-loop baseline, the repair
aggregate is higher, so Rust-relative density did not improve on this run.

The density miss is driven by primer cost, not generated-output volume. Phase 4
generated `20,157` model-output tokens across all turns, below the Phase 3
core-language repair run's `42,314` and the Phase 3 library-mediated run's
`24,367`. The current end-to-end metric charges the 22,157-token primer once
per model call, so 58 calls dominate the aggregate.

## Interpretation

Phase 4 Stage 8 supports freezing with a mixed result:

- Fluency and repair behavior improved enough to close the phase.
- Rust-relative density did not improve under the current metric and is
  recorded as a strategic finding in ADR 0075.
- No sealed-corpus contents, paths, or task metadata were accessed for this
  open-corpus re-evaluation note.
