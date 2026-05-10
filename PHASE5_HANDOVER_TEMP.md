# Phase 5 Handover

Use this in a fresh session for a blind Phase 5 Stage 2 rerun.

## Current state

- Stage 1 is complete.
- Stage 2 is intentionally back to `Pending` in `plans/phase-5-plan.md`.
- The previous non-blind Stage 2 result artifacts were removed.
- A Phase 5 adapter now exists at `corpus/harness/src/tacit_corpus/phase5_eval.py`.
- The CLI entry point is `phase5-eval` from `corpus/harness/`.

## Important constraints

- Do not read, list, search, or otherwise access `corpus/sealed/`.
- Treat this rerun as blind with respect to prior benchmark solutions in the
  repo. Do not consult deleted Stage 2 outputs or recreate them from memory.
- The benchmark definition includes a corrected `r4-map-destination` input.
  That correction is part of Stage 1 and should remain.

## Benchmark inputs

- Spec: `plans/phase-5-benchmark/README.md`
- Manifest: `plans/phase-5-benchmark/manifest.json`

## Adapter usage

From `corpus/harness/`:

```bash
uv run phase5-eval --model <MODEL_ID>
```

Optional:

```bash
uv run phase5-eval --model <MODEL_ID> --tasks r1-record-total,r2-closure-offset
```

## What the adapter does

- Includes the Tacit-Lite primer.
- Uses the authoring-surface benchmark inputs (`main.taca`).
- Reuses the existing model-call and repair-loop helpers.
- Supports up to 2 repair turns.
- Records per-task prompts, raw responses, diagnostics, inspection output,
  generated `.taca`, and run results under `plans/phase-5-results/<run-id>/`.

## Validation already done

From `corpus/harness/`:

```bash
uv run pytest tests/test_phase5_eval.py tests/test_eval.py
uv run phase5-eval --help
```

## Recommended next step

Run Stage 2 from a fresh session with this adapter, then write the new Stage 2
results note from the generated artifacts.
