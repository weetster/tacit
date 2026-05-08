# Phase 4 Results Note

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
- One-shot task pass rate: `80.9%`
- Final task pass rate after repair: `100.0%`
- One-shot compile pass rate: `91.5%`
- One-shot typecheck pass rate: `93.6%`
- Repair-loop recovery rate: `100.0%`
- Average model calls per task: `1.23`
- Primer tokens: `22,157`

## Density

The token counter reports the open-corpus Rust reference total at `7,064`
tokens and the open Tacit reference total at `42,376` tokens, or about `6.00x`
Rust.

For this paid repair-loop run, the model-output Tacit totals were:

- One-shot: `1,057,570` Tacit tokens, or `149.7x` the Rust reference total
- Repair aggregate: `1,305,263` Tacit tokens, or `184.7x` the Rust reference total

Compared with the recorded Phase 3 open repair-loop baseline, the repair
aggregate is higher, so Rust-relative density did not improve on this run.
