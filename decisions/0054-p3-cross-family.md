# 0054 — Phase 3 cross-family evaluation sub-track scope

**Status:** Accepted
**Date:** 2026-04-28
**Phase:** 3, Stage 1
**Closes:** [phase-3-plan.md Q-P3-8](../plans/phase-3-plan.md);
[tacit-plan.md § Phase 3](../plans/tacit-plan.md) cross-family deferral

## Context

[tacit-plan.md § Phase 3](../plans/tacit-plan.md) commits the
cross-family sub-track to "at least one Claude, one GPT, and one
strong open-weight model." The Phase 3 primary track under
[ADR 0052](0052-p3-eval-model-contract.md) already runs Sonnet
(`claude-sonnet-4-6`) and Haiku (`claude-haiku-4-5`), satisfying the
Claude axis. This ADR scopes the additional GPT and open-weight
runs.

The cross-family question is not "does GPT pass the gate?" — the
gate is Sonnet-specific by parent-plan design. The cross-family
question is **whether the primer generalises across model
families.** Three failure modes are interesting:

1. **Tokenizer pathology.** The primer is authored under tiktoken
   `o200k_base` per [ADR 0051](0051-p3-tacit-token-rule.md); GPT
   models tokenize close to that, but open-weight models (Llama,
   DeepSeek, Qwen) use distinct tokenizer families with different
   vocabulary distributions. A primer that compresses well under
   `o200k_base` may compress poorly under another vocabulary,
   shifting the effective primer length the model sees.
2. **Idiom-transfer pathology.** The primer demonstrates Tacit-Lite
   via a specific authoring style. A model trained heavily on, say,
   Haskell-style functional code may pattern-match Tacit-Lite to
   Haskell idioms and produce systematically wrong programs that
   compile-fail in identical ways. A model trained on more imperative
   substrate may have a different failure mode. Cross-family runs
   surface these patterns.
3. **Primer-format pathology.** The primer's section structure,
   fenced-block tag, and one-line output preamble are an Anthropic-
   adjacent format. A different family may need a different
   prompt format to read the primer reliably. The Q-P3-8 deferral
   asks whether one shared primer suffices or per-family variants
   are needed.

The user's Stage 1 design pass commits to **one shared primer
initially**, with the explicit fallback that a material cross-
family regression triggers a primer-portability ADR rather than a
Phase 3 fail.

The user's Stage 1 model picks: **GPT 5.5 via OpenRouter** for the
GPT axis and **DeepSeek V3.1 via OpenRouter** for the open-weight
axis (with **Qwen 3 Coder** as a fallback if V3.1 is unavailable).
OpenRouter provides a unified API surface for both, simplifying the
harness's third-party-provider integration.

## Decision

**The cross-family sub-track adds two models to the primary Sonnet
+ Haiku run: `openai/gpt-5.5` and `deepseek/deepseek-v3.1` (Qwen
fallback `qwen/qwen-3-coder-32b`), both invoked via OpenRouter. All
four models share one primer for Phase 3's first cross-family pass.
A material regression (defined below) triggers a per-family primer
ADR; absent regression, one primer holds.**

### Model set

| Family   | Role               | Provider     | Model ID (working)                  |
|----------|--------------------|--------------|-------------------------------------|
| Claude   | Primary gate       | Anthropic    | `claude-sonnet-4-6`                 |
| Claude   | Primary report     | Anthropic    | `claude-haiku-4-5`                  |
| GPT      | Cross-family       | OpenRouter   | `openai/gpt-5.5`                    |
| Open     | Cross-family       | OpenRouter   | `deepseek/deepseek-v3.1`            |
| Open (fallback) | If V3.1 unavailable | OpenRouter | `qwen/qwen-3-coder-32b`         |

The exact OpenRouter model IDs are recorded in `run.json` per
[ADR 0052 § Reproducibility metadata](0052-p3-eval-model-contract.md)
at run time. OpenRouter's published IDs may diverge from the
working-name table above; the harness records whatever it actually
called. The fallback rule is operator-triggered: if DeepSeek V3.1
is materially unavailable on OpenRouter at Stage 10 entry, Qwen 3
Coder substitutes and the swap is logged.

### Primer-portability rule

- **Phase 3 first pass uses one shared primer** at
  `plans/primer/tacit-lite-primer.md`. All four models receive
  identical text; the primer-fixture invariants from
  [ADR 0050 § Authoring discipline](0050-p3-primer-scope.md) hold
  across families.
- **Per-family primer variants are not authored speculatively.**
  If the shared primer works for all four, the primer-portability
  thesis is supported and no further work is needed.
- **A material regression triggers a per-family primer ADR.** The
  regression definition below names the threshold; once tripped,
  the response is a Q-P3-8 follow-up ADR ("primer-portability:
  per-family variants for <model>") rather than a Phase 3 fail.

### Material regression threshold

A cross-family run is a "material regression" if **any** of:

1. The non-Anthropic family's pass rate on the primary corpus is
   **more than 20 percentage points below Sonnet's pass rate** on
   the same corpus.
2. The non-Anthropic family's end-to-end token cost is **more than
   1.5× Sonnet's token cost** on the non-stdlib-dominated aggregate
   per [ADR 0021](0021-corpus-stdlib-dominance-reporting.md).
3. The non-Anthropic family's pass rate on the maintenance
   sub-track per [ADR 0053](0053-p3-maintenance-subtrack.md) is
   **more than 30 percentage points below Sonnet's**.

Below those thresholds, the cross-family result is a data point and
the run is reported as-is. At or above any threshold, a primer-
portability follow-up ADR is opened and Phase 3's freeze ADR (per
[phase-3-plan.md § Stage 11](../plans/phase-3-plan.md)) records the
opening as a known follow-up. The threshold values are calibrated
to be wide enough to absorb routine cross-family variance (10–15
pp pass-rate gap, modest token-cost overhead) and narrow enough to
flag a primer that fails to teach a family.

### Sampling and context

Cross-family runs use the same sampling parameters as primary runs
per [ADR 0052 § Sampling parameters](0052-p3-eval-model-contract.md):
`temperature=0`, `max_tokens=8192`, no `top_p` override. The same
context construction (`system: primer`, `user: task_statement`)
applies. The harness's OpenRouter integration uses the OpenAI-
compatible Chat Completions endpoint that OpenRouter exposes, with
the model ID passed through.

OpenRouter's `system` field on Chat Completions does not support
prompt caching equivalent to the Anthropic Messages API's
ephemeral cache. Per-task API cost is therefore higher in absolute
terms for cross-family runs (the primer is reprocessed each call);
this does not affect the Phase 3 token-count gate, which is
measured under `o200k_base` per ADR 0051. The cost overhead is
absorbed by the operator-triggered run budget.

### Output extraction

Output extraction follows
[ADR 0052 § Output extraction](0052-p3-eval-model-contract.md): the
first ` ```tacit ` fenced block in the response. Cross-family
models that wrap output differently (GPT's tendency to add
explanatory prose; Qwen's occasional Markdown variation) are
handled identically — the harness extracts the first block and
treats anything outside it as commentary.

If extraction fails systematically across a family (e.g., a model
returns code without fences), the primer-portability follow-up
described above is the intended response, not an extraction-rule
relaxation.

### API key handling

OpenRouter access uses `OPENROUTER_API_KEY` per
[ADR 0052 § API key handling](0052-p3-eval-model-contract.md). The
harness reads the key from environment, not from a file. A missing
key is a hard error before any request.

### Run scope

Each cross-family model runs:

1. The open 47 corpus tasks. (Cross-family open run.)
2. The sealed 13 corpus tasks. (Cross-family sealed run, requires
   `--include-sealed`.)
3. The maintenance sub-track per
   [ADR 0053](0053-p3-maintenance-subtrack.md). (Cross-family
   maintenance run.)

That is 4 models × 3 scopes = 12 runs total in Stage 9–10. Each
run produces a `run.json` and a metric file per
[ADR 0055](0055-p3-metrics-schema.md). All metric files commit
under `plans/phase-3-results/`.

### Reporting

The Stage 10 results README at `plans/phase-3-results/README.md`
gains a cross-family table:

```
              open    sealed   maintenance   token-cost-vs-sonnet
sonnet-4-6     —       —         —              1.00x  (baseline)
haiku-4-5      …       …         …              …
gpt-5.5        …       …         …              …
deepseek-v3.1  …       …         …              …
```

Each cell carries the pass rate; the right column carries the
cross-family token-cost ratio relative to Sonnet on the
non-stdlib-dominated aggregate.

### Reported, not gating

Per [phase-3-plan.md § Exit criteria](../plans/phase-3-plan.md), the
cross-family sub-track is **reported alongside but not part of the
go/no-go decision.** A cross-family fail on a primary Sonnet pass
opens a primer-portability follow-up ADR; it does not block Phase 3
freeze. The parent plan's Phase 5 path (synthetic corpus + fine-
tuning) remains the long-run remedy if cross-family results are
systematically poor.

## Alternatives considered

- **Use Anthropic / OpenAI / open-weight providers directly** rather
  than OpenRouter. Rejected. Three providers means three API
  integrations, three credential paths, three rate-limit handlers.
  OpenRouter unifies the second and third; the harness already
  speaks Anthropic Messages directly for Sonnet/Haiku. Two
  integrations is the minimum.
- **Author per-family primer variants up front.** Rejected. A
  speculative variant is a Q-P3-4-style cap-busting move per
  [ADR 0050 § Compactness discipline](0050-p3-primer-scope.md);
  per-family work multiplies the authoring cost without evidence
  that it's needed. The shared-primer-first posture is the load-
  bearing decision; variants are the *response* to a regression,
  not a hedge.
- **Skip the cross-family sub-track entirely.** Rejected. The parent
  plan lists it as a deliverable; skipping is a scope reduction.
- **Gate Phase 3 on cross-family results.** Rejected. The primary
  gate is Sonnet-specific by parent-plan design; cross-family is a
  generalisation signal, not a primary thesis. Gating both axes
  collapses two distinct measurements into a brittle composite.
- **Tighten the regression threshold to 10 pp.** Rejected as too
  noisy. Routine cross-family variance is roughly 10–15 pp on
  programming benchmarks; a 10 pp threshold would trigger a primer-
  portability follow-up on every run.
- **Loosen the threshold to 30 pp.** Rejected as too forgiving. A
  30 pp gap means the primer half-fails for the family, which is
  the failure mode the threshold is supposed to flag.
- **Pick `qwen/qwen-3-coder-32b` over `deepseek/deepseek-v3.1` as
  the primary open-weight.** Rejected at Stage 1 design — DeepSeek
  V3.1 has stronger generalist code performance per public
  benchmarks at the time of this ADR; Qwen 3 Coder is the
  fallback. The choice is reversible: if V3.1 underperforms on
  Stage 9–10 runs, the primer-portability follow-up may name Qwen
  as the substitute.
- **Use multiple GPT versions** (e.g., 5.5 + 4.1) for cross-version
  comparison within the GPT family. Rejected for Phase 3. One per
  family is the parent-plan minimum; within-family comparison is a
  Phase 4+ extension.
- **Use Haiku as the cross-family Claude representative** rather
  than Sonnet. Rejected — Sonnet is the primary gate target;
  comparing other families against Haiku would understate Sonnet's
  position. Sonnet is the cross-family yardstick.

## Consequences

- **Stage 10 has a fixed model set.** Two new models, one provider,
  one new credential. The harness gains an OpenRouter integration
  but no new abstraction over model providers — it speaks
  Anthropic Messages and OpenRouter Chat Completions directly.
- **Cross-family signal is bounded.** Three thresholds tell Stage 11
  whether to open a primer-portability follow-up. Per-family
  primer authoring is on demand only.
- **Eval cost rises proportionally.** Twelve runs (four models ×
  three scopes), with the OpenRouter half lacking the prompt-cache
  benefit. The operator's run budget absorbs this.
- **The primer-portability thesis is testable.** Phase 3's first
  cross-family pass either supports "one primer suffices" or names
  which family it doesn't suffice for. Either outcome is useful
  for Phase 4–5 planning.
- **OpenRouter is a dependency.** OpenRouter availability of the
  pinned models is a load-bearing assumption for Stage 10. The
  Qwen fallback is the operator-side mitigation; if both V3.1 and
  Qwen 3 Coder are unavailable simultaneously, the open-weight axis
  is reported as "unavailable at run time" and a follow-up ADR
  picks a replacement.
- **This ADR freezes with Stage 1.** The model set, threshold
  definitions, and primer-portability rule are pinned; OpenRouter
  ID strings are recorded at run time and may differ from the
  working-name table.

## Related decisions

- [ADR 0021](0021-corpus-stdlib-dominance-reporting.md) — token-
  cost regression threshold uses the non-stdlib-dominated
  aggregate.
- [ADR 0050](0050-p3-primer-scope.md) — primer source-of-truth;
  cross-family runs all use the same primer at first pass.
- [ADR 0051](0051-p3-tacit-token-rule.md) — token-count rule;
  applies uniformly across families.
- [ADR 0052](0052-p3-eval-model-contract.md) — model contract;
  this ADR extends with provider = OpenRouter for non-Anthropic
  models.
- [ADR 0053](0053-p3-maintenance-subtrack.md) — maintenance
  sub-track; cross-family runs cover it too.
- [ADR 0055](0055-p3-metrics-schema.md) — metric schema; per-family
  metric files share format.
- [phase-3-plan.md § Stage 10, § Exit criteria, § Risks](../plans/phase-3-plan.md)
  — implementation surface, reported-not-gating posture, and
  cross-family-portability risk.
- [tacit-plan.md § Phase 3](../plans/tacit-plan.md) — parent
  deferral this ADR closes.
