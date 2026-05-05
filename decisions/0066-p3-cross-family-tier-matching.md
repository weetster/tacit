# 0066 — Phase 3 cross-family model tier matching

**Status:** Accepted
**Date:** 2026-05-04
**Phase:** 3, Stage 10 (pre-run amendment)
**Amends:** [ADR 0054](0054-p3-cross-family.md) § Model set, § Material
regression threshold, § Run scope, § Reporting
**Closes:** primer-portability calibration follow-up surfaced after
Stage 9 (library-mediated open run `019df533`)

## Context

[ADR 0054](0054-p3-cross-family.md) pinned the cross-family sub-track
to `openai/gpt-5.5` (GPT axis) and `deepseek/deepseek-v3.1`
(open-weight axis), both via OpenRouter, with `qwen/qwen-3-coder-32b`
as the open-weight fallback. The Phase 3 primary gate runs on
Sonnet (`claude-sonnet-4-6`) per
[ADR 0052](0052-p3-eval-model-contract.md), with Haiku
(`claude-haiku-4-5`) reported alongside.

The cross-family question per ADR 0054 § Context is **primer
portability**, not raw capability. The regression threshold
(>20 pp pass-rate gap, >1.5× token cost, >30 pp maintenance gap) is
calibrated against Sonnet under the implicit assumption that the
cross-family comparator is a Sonnet-tier model. This implicit
assumption is the bug.

GPT-5.5 is positioned by OpenAI's tiering as an Opus peer, not a
Sonnet peer. Pairing Sonnet against an Opus-tier comparator
collapses two distinct axes into one measurement and makes the
regression threshold uninterpretable in both directions:

- **Within threshold (≤20 pp gap).** A capability-stronger model
  that lands close to Sonnet may have absorbed a weaker primer; the
  result is consistent with the primer being non-portable. No
  signal.
- **Beats Sonnet outright.** A capability-stronger model that
  outperforms Sonnet tells us nothing about primer portability;
  it's a capability gap masquerading as a portability win. No
  signal.
- **Below threshold (>20 pp gap).** A capability-stronger model
  that *still* underperforms Sonnet by >20 pp is a clear primer
  failure — but this outcome is the least likely one and its
  absence does not falsify the portability hypothesis.

Tier-matched comparators restore the ADR 0054 threshold's
interpretability. A Sonnet-tier GPT model (GPT-5.4) and a Haiku-tier
GPT model (GPT-5.4-mini) let the threshold flag *primer* failures
distinctly from *capability* gaps. DeepSeek V3.1 already sits in the
Sonnet/main-tier band on public coding benchmarks at the time of
this ADR, so the open-weight axis stays as ADR 0054 set it.

The amendment is being made between Stage 9 (Sonnet primary +
library-mediated runs done) and Stage 10 (Haiku, maintenance,
cross-family) — before any cross-family run has been spent. No paid
Stage 10 work is invalidated.

## Decision

**The Phase 3 cross-family model set is replaced with tier-matched
pairs. The GPT axis runs `openai/gpt-5.4` against Sonnet and
`openai/gpt-5.4-mini` against Haiku. The open-weight axis stays as
ADR 0054 set it: `deepseek/deepseek-v3.1` against Sonnet, with
`qwen/qwen-3-coder-32b` as the operator-triggered fallback. The
regression threshold from ADR 0054 § Material regression threshold
is restated against each cross-family model's matched-tier Anthropic
baseline rather than always against Sonnet. GPT-5.5 is removed from
the Phase 3 model set.**

### Model set (replaces ADR 0054 § Model set)

| Family   | Tier   | Role                  | Provider     | Model ID (working)              | Matched baseline       |
|----------|--------|-----------------------|--------------|---------------------------------|------------------------|
| Claude   | Main   | Primary gate          | Anthropic    | `claude-sonnet-4-6`             | — (baseline)           |
| Claude   | Small  | Primary report        | Anthropic    | `claude-haiku-4-5`              | — (baseline)           |
| GPT      | Main   | Cross-family (Sonnet peer) | OpenRouter | `openai/gpt-5.4`             | `claude-sonnet-4-6`    |
| GPT      | Small  | Cross-family (Haiku peer)  | OpenRouter | `openai/gpt-5.4-mini`        | `claude-haiku-4-5`     |
| Open     | Main   | Cross-family (Sonnet peer) | OpenRouter | `deepseek/deepseek-v3.1`     | `claude-sonnet-4-6`    |
| Open (fallback) | Main | If V3.1 unavailable | OpenRouter | `qwen/qwen-3-coder-32b`        | `claude-sonnet-4-6`    |

GPT-5.5 is **not** in the Phase 3 model set. Opus-tier
cross-family evaluation is a Phase 4+ scope question and is not
prejudged here.

The exact OpenRouter model IDs are recorded in `run.json` per
[ADR 0052 § Reproducibility metadata](0052-p3-eval-model-contract.md)
at run time. OpenRouter's published IDs may diverge from the
working-name table above; the harness records whatever it actually
called. The fallback rule from ADR 0054 (`deepseek-v3.1` →
`qwen-3-coder-32b` if V3.1 is materially unavailable at Stage 10
entry) is unchanged.

### Material regression threshold (amends ADR 0054)

A cross-family run is a "material regression" if **any** of the
following holds against the model's **matched-tier Anthropic
baseline** (per the Model set table above):

1. The cross-family model's pass rate on the primary corpus is
   **more than 20 percentage points below its matched baseline's
   pass rate** on the same corpus.
2. The cross-family model's end-to-end token cost is **more than
   1.5× its matched baseline's token cost** on the non-stdlib-
   dominated aggregate per
   [ADR 0021](0021-corpus-stdlib-dominance-reporting.md).
3. The cross-family model's pass rate on the maintenance sub-track
   per [ADR 0053](0053-p3-maintenance-subtrack.md) is **more than
   30 percentage points below its matched baseline's**.

Below those thresholds, the cross-family result is a data point
and the run is reported as-is. At or above any threshold, a
primer-portability follow-up ADR is opened per ADR 0054's
unchanged response rule. The numeric values (20 pp / 1.5× / 30 pp)
are deliberately preserved from ADR 0054; only the comparator
changes.

The Sonnet-vs-Haiku comparison is **not** itself a cross-family
data point — both are baselines. Within-Anthropic tier difference
is reported under the existing Stage 9 rollup, not as a portability
signal.

### Run scope (amends ADR 0054 § Run scope)

Each of the **three** cross-family models (GPT-5.4, GPT-5.4-mini,
DeepSeek V3.1) runs:

1. The open 47 corpus tasks. (Cross-family open run.)
2. The sealed 13 corpus tasks. (Cross-family sealed run, requires
   `--include-sealed`.)
3. The maintenance sub-track per
   [ADR 0053](0053-p3-maintenance-subtrack.md).

That is **3 cross-family models × 3 scopes = 9 cross-family runs**,
on top of the **2 baseline models × 3 scopes = 6 baseline runs**,
for **15 runs total** in Stage 9–10. ADR 0054's original
4 × 3 = 12 figure is superseded.

The cost increase relative to ADR 0054 is one additional model
(GPT-5.4-mini) × three scopes = three additional OpenRouter runs.
The operator's run budget absorbs this; the small-tier model is
the cheapest cross-family addition and is the load-bearing piece
of the Haiku-axis portability question.

### Reporting (amends ADR 0054 § Reporting)

The Stage 10 results README at `plans/phase-3-results/README.md`
gains a tier-grouped cross-family table (replaces the flat table
in ADR 0054):

```
Main tier (vs claude-sonnet-4-6)
                  open    sealed   maintenance   token-cost-vs-baseline
sonnet-4-6         —       —         —              1.00x  (baseline)
gpt-5.4            …       …         …              …
deepseek-v3.1      …       …         …              …

Small tier (vs claude-haiku-4-5)
                  open    sealed   maintenance   token-cost-vs-baseline
haiku-4-5          —       —         —              1.00x  (baseline)
gpt-5.4-mini       …       …         …              …
```

Each cell carries the pass rate; the right column carries the
cross-family token-cost ratio relative to the **matched-tier
baseline** on the non-stdlib-dominated aggregate. A model that
trips its tier's regression threshold is flagged inline with the
follow-up ADR ID once opened.

### Reported, not gating (unchanged from ADR 0054)

The cross-family sub-track remains reported alongside but not part
of the go/no-go decision per
[phase-3-plan.md § Exit criteria](../plans/phase-3-plan.md). A
matched-tier cross-family fail opens a primer-portability follow-up
ADR; it does not block Phase 3 freeze. The parent plan's Phase 5
path (synthetic corpus + fine-tuning) remains the long-run remedy
if cross-family results are systematically poor across tiers.

### Sampling, context, output extraction, API key handling

Unchanged from
[ADR 0052 §§ Sampling parameters, Context construction, Output
extraction, API key handling](0052-p3-eval-model-contract.md) and
[ADR 0054 §§ Sampling and context, Output extraction, API key
handling](0054-p3-cross-family.md). Tier matching is a comparator
change, not a contract change.

## Alternatives considered

- **Keep ADR 0054's GPT-5.5 pick and reinterpret the threshold
  per-model.** Rejected. The threshold's interpretability requires
  capability parity; per-model reinterpretation is bespoke
  measurement, not a portability test. If we want an Opus-tier
  cross-family run, that is a separate question with its own
  threshold, scoped to a different phase.
- **Keep GPT-5.5 *and* add GPT-5.4 / GPT-5.4-mini.** Rejected for
  Phase 3. Three GPT models triples the GPT-axis cost without a
  Phase 3 thesis to justify it. The Opus-tier comparator is a
  Phase 4+ extension; Phase 3 should answer the matched-tier
  portability question first.
- **Add only GPT-5.4 (skip the small-tier pair).** Rejected. The
  Haiku-tier portability question is the more interesting one
  empirically — small-tier models compress capability hardest, so
  a primer that teaches Haiku may fail GPT-5.4-mini in distinct
  ways. Skipping the small tier loses half the matched-tier
  signal.
- **Replace DeepSeek V3.1 with a small-tier open-weight (e.g., a
  Qwen-3 distill).** Rejected. The open-weight axis is already
  thin (one model per ADR 0054 with one fallback); collapsing it
  to a small-tier-only pair would lose the main-tier open-weight
  comparator entirely. Small-tier open-weight models are a
  legitimate Phase 4 extension if the main-tier result motivates
  it.
- **Author per-tier primer variants up front.** Rejected on the
  same grounds as ADR 0054's "per-family variants" rejection: the
  shared-primer-first posture is load-bearing; tier variants are
  the *response* to a regression, not a hedge.
- **Use both `gpt-5.4` and `gpt-5.4-turbo` (or similar
  intra-version variants).** Rejected for Phase 3. One model per
  tier per family is the parent-plan minimum; intra-version
  comparison is Phase 4+.

## Consequences

- **ADR 0054's model set table is superseded.** ADR 0054 stays
  Accepted; this ADR amends the named sections. The
  primer-portability response rule, OpenRouter dependency, and
  reported-not-gating posture from ADR 0054 are unchanged.
- **Stage 10 plans for 15 runs, not 12.** Three additional
  OpenRouter runs (GPT-5.4-mini × three scopes). The operator's
  run budget absorbs this.
- **Tier matching makes the threshold interpretable.** A
  cross-family fail under the amended threshold is a primer-
  portability signal, not a capability artifact. A pass is
  evidence the primer travels across families at the same
  capability tier.
- **Opus-tier cross-family is explicitly deferred.** If the
  matched-tier runs all pass, the Opus-tier question can be
  picked up in Phase 4+ with a fresh ADR; if they fail, primer
  redesign per ADR 0054's response rule is the priority and
  Opus-tier comparison is moot until the primer is fixed.
- **Phase 3 freeze ADR (Stage 11) records this amendment.** The
  Stage 11 ADR per
  [phase-3-plan.md § Stage 11](../plans/phase-3-plan.md) lists
  ADRs 0054 and 0066 together as the cross-family decision pair.

## Related decisions

- [ADR 0021](0021-corpus-stdlib-dominance-reporting.md) —
  non-stdlib-dominated aggregate; threshold scope unchanged.
- [ADR 0050](0050-p3-primer-scope.md) — primer source-of-truth;
  shared across all five Phase 3 models.
- [ADR 0051](0051-p3-tacit-token-rule.md) — token-count rule;
  applies uniformly across families and tiers.
- [ADR 0052](0052-p3-eval-model-contract.md) — model contract;
  Anthropic baselines; this ADR adds tier-matched cross-family
  pairs without changing the contract shape.
- [ADR 0053](0053-p3-maintenance-subtrack.md) — maintenance
  sub-track; cross-family runs cover it per matched-tier baseline.
- [ADR 0054](0054-p3-cross-family.md) — cross-family decision this
  ADR amends.
- [ADR 0055](0055-p3-metrics-schema.md) — metric schema; per-model
  metric files share format. The matched-baseline pairing is a
  reporting convention, not a schema change.
- [phase-3-plan.md § Stage 10, § Exit criteria](../plans/phase-3-plan.md)
  — implementation surface and reported-not-gating posture; the
  Stage 10 description gains a pointer to this ADR.
