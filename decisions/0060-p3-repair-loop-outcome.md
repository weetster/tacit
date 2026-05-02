# 0060 — Phase 3 repair-loop outcome and next direction

**Status:** Accepted
**Date:** 2026-05-02
**Phase:** 3
**Amends:** [ADR 0055](0055-p3-metrics-schema.md) for repair-loop token
accounting and repair-specific reporting gates.

## Context

The Phase 3 standalone one-shot Sonnet runs did not clear the parent-plan
gate. The best standalone one-shot open result was 29/47 tasks, below the
33/47 open-task threshold that corresponds to >70%. Later primer growth
regressed to 24/47.

The proposed repair-loop experiment then ran with the best-run primer
(`019de600-a048-7beb-85d5-648bccd6fea3`, 15,533 tokens), the same model
(`claude-sonnet-4-6`), and up to two repair turns per task. The full open
repair-loop run `019de6ef-e75e-70d8-aa52-e98c4c577f7d` produced:

- 30/47 one-shot task passes.
- 40/47 final task passes after repair.
- 10/17 initially failed tasks repaired.
- 5/11 invalid-output failures repaired to full task pass.
- 5/6 behavioral failures repaired to full task pass.
- 1.53 average model calls per task.

The repair-loop result is materially better than one-shot prompting, but it is
a different claim from "the model writes correct Tacit from the primer alone."

## Decision

Phase 3 is not frozen as passed. The one-shot primer-only result remains a
failure against the current correctness and token gates.

The repair-loop result is accepted as useful evidence for an agentic
write-check-repair direction. It must be reported separately from the
one-shot primary result and must not be merged into the Phase 3 primary gate.

The evaluation harness now reports repair-loop token economics explicitly:

- `repair_primer_tokens_total` — primer tokens multiplied by model calls.
- `repair_tacit_tokens_total` — repair primer tokens plus all generation
  tokens.
- `python_tokens_total` — Python baseline tokens for the same task set.
- `repair_token_delta` — repair-loop Tacit cost relative to Python.
- `repair_primer_amortized_total` — diagnostic primer-once repair total.

The harness also emits repair-specific reporting gates:

- `repair_final_pass_rate_gate` at 70%.
- `repair_invalid_recovery_gate` at 50%.
- `repair_behavioral_recovery_gate` at one third.
- `repair_promising_overall`, a reporting boolean over those repair gates.

These repair gates do not replace `passed_overall`, which remains the primary
Phase 3 one-shot verdict.

## Consequences

- Do not spend more on full open one-shot reruns without changing the
  experiment.
- Do not proceed to sealed repair-loop runs until a sealed-safe feedback
  redaction policy is written.
- Do not proceed to Haiku or cross-family baselines until the open Sonnet path
  has a clearer target.
- Treat standard-library expansion as a separate library-mediated authoring
  hypothesis, not as a continuation of primer-only core-language tuning.
- Keep token accounting conservative: every repair turn pays the primer under
  the reported repair token total.

## Alternatives considered

- **Count repair-loop final passes as Phase 3 primary passes.** Rejected. The
  parent-plan gate is primer-only one-shot authoring; adding compiler/test
  feedback changes the evaluated workflow.
- **Ignore token economics during repair-loop reporting.** Rejected. The
  repair loop adds model calls, so reporting only final correctness would hide
  the largest cost of the workflow.
- **Continue primer growth.** Rejected for now. The recorded data shows
  diminishing and then negative returns, while token cost worsens with primer
  size.
- **Jump directly to sealed repair.** Rejected. Open repair prompts include
  concrete failing test details; sealed repair needs a redaction policy first.

## Related

- [Phase 3 results note](../plans/phase-3-results/README.md)
- [Phase 3 repair-loop experiment](../plans/phase-3-repair-loop-experiment.md)
- [Phase 3 stdlib next steps](../plans/phase-3-stdlib-next-steps.md)
