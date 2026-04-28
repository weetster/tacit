# 0052 — Phase 3 evaluation harness model invocation contract

**Status:** Accepted
**Date:** 2026-04-28
**Phase:** 3, Stage 1
**Closes:** [phase-3-plan.md Q-P3-6](../plans/phase-3-plan.md)

## Context

[phase-3-plan.md § Stage 8](../plans/phase-3-plan.md) builds a
`corpus-eval` harness command that drives a model with the Phase 3
primer in context, captures generated Tacit-Lite, compiles it, runs
the test cases, and records metrics. The Q-P3-6 deferral asks for
the contract under which the model is invoked: which model
identifiers count as "Sonnet" and "Haiku" for the gate, what
sampling parameters are used, what context the harness assembles,
and what reproducibility metadata is captured.

A reproducible eval has four moving parts:

1. **Model identity.** Anthropic's `claude-sonnet-*` and
   `claude-haiku-*` are version families, not single endpoints. A
   gate that says "Sonnet > 70%" without naming the version is
   unreproducible across version updates.
2. **Sampling.** Temperature, top-p, max tokens, and stop sequences
   together determine output distribution. Two runs with different
   sampling parameters are not the same evaluation.
3. **Context construction.** The model sees `(primer, task_statement)`
   per the parent plan, but the boundary between system message and
   user message — and the question of whether the test cases or the
   Python reference leak into either — is unspecified. A leak makes
   the eval trivial; a missing field makes it fail spuriously.
4. **Metadata.** Without a `run.json` per run, the metric file is
   uninterpretable in six months — which model, which primer,
   which harness commit produced these numbers?

The user's Stage 1 design pass picked the model identifiers
(`claude-sonnet-4-6`, `claude-haiku-4-5`) and deferred sampling to
this ADR. Sampling defaults are settled here in favour of
reproducibility (temperature 0) over diversity, with a flake budget
captured as the per-task ±2-case tolerance from
[phase-3-plan.md § Risks](../plans/phase-3-plan.md).

## Decision

**The harness invokes Anthropic's Messages API directly. Sonnet is
`claude-sonnet-4-6`, Haiku is `claude-haiku-4-5`. Sampling is
temperature 0, max output 8192 tokens, no top-p override.
Context is `system: primer`, `user: task_statement` only — never
test cases, never `reference.py`. Each run writes a `run.json`
metadata file alongside the metric output.**

### Model identifiers

| Role     | Model ID                | Provider                  |
|----------|-------------------------|---------------------------|
| Sonnet   | `claude-sonnet-4-6`     | Anthropic Messages API    |
| Haiku    | `claude-haiku-4-5`      | Anthropic Messages API    |

Cross-family models (GPT 5.5, an open-weight) are scoped by
[ADR 0054](0054-p3-cross-family.md) and use OpenRouter, not the
Anthropic API. The Q-P3-6 contract above governs only the primary
Sonnet / Haiku gate runs.

### Sampling parameters

| Parameter      | Value | Rationale                                               |
|----------------|-------|---------------------------------------------------------|
| `temperature`  | `0`   | Reproducibility primacy. Per-task flake budget is ±2 cases per [phase-3-plan.md § Risks](../plans/phase-3-plan.md). |
| `top_p`        | unset | Default; not overridden when `temperature = 0`.         |
| `max_tokens`   | `8192`| Largest reference under ADR 0048 idiom rules is well under 1k tokens; 8k is generous.|
| `stop_sequences`| `[]` | No stop sequences. Output extraction is by fenced block; truncation by `max_tokens` is a hard failure. |

`temperature = 0` does not guarantee bit-identical reproducibility
across model-server replicas; the Anthropic API documents low-level
nondeterminism even at zero temperature. The ±2-case tolerance is
the response.

### Context construction

The harness assembles each request as:

```python
messages.create(
    model=<id>,
    system=PRIMER_TEXT,           # raw primer markdown, no edits
    max_tokens=8192,
    temperature=0,
    messages=[
        {"role": "user", "content": TASK_STATEMENT},
    ],
)
```

`PRIMER_TEXT` is the verbatim contents of
`plans/primer/tacit-lite-primer.md` (per
[ADR 0050](0050-p3-primer-scope.md)).

`TASK_STATEMENT` is the verbatim contents of
`corpus/tasks/<category>/<NNN-slug>/task.md`.

**No other content** is included. The user message is the task
statement and nothing else. Specifically excluded:

- `tests.jsonl` — test cases. Including them collapses the eval
  into "model copies expected output."
- `reference.py` — the Python reference. Including it gives the
  model the answer in another language; ADR 0019's
  baseline-purity discipline forbids this.
- `reference.tac` — the Tacit-Lite reference. Including it is
  worse than the Python case (it gives the answer in the target
  language).
- Any cross-task hints, prior-task results, or conversation
  history. Each task is a fresh request.

### Output extraction

The model is instructed via a one-line preamble appended to the
task statement:

```
{TASK_STATEMENT}

Write the solution as a single Tacit-Lite program in a fenced
block: ```tacit ... ```. Do not include the sidecar.
```

The harness extracts the first ` ```tacit ` fenced block from the
response. Multiple fenced blocks are an extraction error. Zero
fenced blocks is an extraction error. Truncation (response stops
mid-block due to `max_tokens`) is an extraction error. Extraction
errors are recorded as compile-failures-with-cause-extraction
in the metric file ([ADR 0055](0055-p3-metrics-schema.md)).

The sidecar requirement: every Tacit-Lite program needs a sidecar
to typecheck (per [ADR 0048](0048-p3-tacit-idiom-rules.md)). The
harness synthesises a minimal sidecar for the model's output:
auto-generated `[types.main]` with the task's expected effect
signature (read from a per-task harness annotation,
`harness-spec.toml`, that the corpus authors maintain alongside
`tests.jsonl`). The model is not asked to produce a sidecar.

### Prompt caching

Per Anthropic Messages API, the `system` field supports prompt
caching with a `cache_control` breakpoint at the end of the system
text. The harness sets this breakpoint:

```python
system=[
    {
        "type": "text",
        "text": PRIMER_TEXT,
        "cache_control": {"type": "ephemeral"},
    }
],
```

Prompt caching reduces the per-task API cost dramatically (the
~10,500-token primer is cache-read for every task after the first
in a run). It does **not** affect the Phase 3 token-count gate:
the gate is measured under `tiktoken o200k_base` per
[ADR 0051](0051-p3-tacit-token-rule.md), independently of API
billing. Caching is a cost optimisation, not a measurement change.

### Retry policy

The harness retries on **transient API errors only** — connection
errors, 5xx server errors, 429 rate limits — with exponential
backoff (1s, 2s, 4s) up to 3 retries. After the third retry
failure, the task is recorded as `api-error` and counted against
the pass rate.

The harness does **not** retry on:

- Successful API responses with un-extractable content.
- Compile or typecheck failures of the extracted Tacit-Lite.
- Test-case failures.

Retrying on a bad output amounts to multi-sample evaluation, which
is a different methodology and not the parent plan's gate. One
sample per task per run.

### Timeout

The harness enforces a 120-second per-request timeout. A timeout
is treated as a transient error and is subject to the retry policy.
The 120-second value is generous against typical Sonnet response
times (~10–30s for the primer-cached case) and absorbs 99th-
percentile latency spikes.

### Reproducibility metadata

Every run writes a `run.json` to the run's output directory:

```json
{
  "harness_git_sha": "<full sha>",
  "corpus_git_sha": "<full sha>",
  "primer_blake3": "<full hex>",
  "primer_token_count": 10487,
  "model_id": "claude-sonnet-4-6",
  "provider": "anthropic",
  "sampling": {
    "temperature": 0,
    "max_tokens": 8192
  },
  "tasks_scope": "open|sealed|all",
  "task_ids": ["arithmetic/001-sum-to-n", "..."],
  "started_at": "2026-MM-DDTHH:MM:SSZ",
  "completed_at": "2026-MM-DDTHH:MM:SSZ",
  "tiktoken_encoding": "o200k_base",
  "harness_version": "0.1.0"
}
```

Filename convention: `<run-id>.run.json`, where `<run-id>` is
`<model_id>_<tasks_scope>_<started_at>` with timestamps in
`YYYYMMDDTHHMMSSZ`. The same `<run-id>` prefixes the metric output
file per [ADR 0055](0055-p3-metrics-schema.md).

### API key handling

The harness reads the API key from the `ANTHROPIC_API_KEY`
environment variable. It does not read from a checked-in file, a
shared secret, or a config-file path. Failure to find the key is
a hard error before any task is dispatched — the harness must not
report partial results when the key is missing.

OpenRouter for [ADR 0054](0054-p3-cross-family.md) cross-family
runs uses `OPENROUTER_API_KEY`, same discipline.

### Sealed handling

Per [ADR 0020](0020-sealing-held-out-in-repo.md), sealed task bodies
live under `corpus/sealed/`. The harness reads sealed `task.md`
contents only when invoked with `--include-sealed`, and the
sealed-task user messages are constructed identically to open-task
user messages. The harness must not log sealed task content to
stdout or to the metric file.

The metric file records pass/fail per sealed task ID; the model's
generated source for sealed tasks is written to a private
`failures/<task>/` tree only when the task fails (for diagnostic
purposes), and that tree is gitignored. Successful sealed-task
generations are discarded after metrics extraction — their content
never enters the repo.

### CI integration

CI runs `corpus-eval --dry-run` per push: this exercises the
harness's primer-loading, task-loading, request-construction, and
metric-writing paths without invoking the paid model API. A real
end-to-end eval is operator-triggered, not CI-triggered.

## Alternatives considered

- **Use `temperature = 0.2` for diversity.** Rejected. Reproducibility
  is the load-bearing property of a gate run; the parent plan's
  Stage 9 baseline gates write a number, and that number must be
  the same on a re-run absent a primer or model change. The flake
  budget of ±2 per task absorbs residual API-side nondeterminism.
- **Multi-sample with majority vote** (sample N times, take the
  most-frequent answer). Rejected. This is a different methodology
  ("pass@k" or "vote@k") and not what the parent plan's gate
  measures. Sonnet > 70% on a single sample is the claim.
- **Include `tests.jsonl` in the user message.** Rejected. Trivially
  collapses the eval — the model can pattern-match expected outputs
  to inputs without ever writing real Tacit-Lite. This is the
  central anti-leakage rule.
- **Include `reference.py` as a "translation hint."** Rejected. The
  Phase 3 thesis is "model writes Tacit-Lite from primer alone";
  including the Python reference in the user message changes the
  thesis to "model translates Python to Tacit-Lite," which is
  weaker and not what the parent plan claims.
- **Write a system prompt that prefixes the primer with extra
  instructions.** Rejected. The primer is the teaching artifact;
  prepending instructions creates a hidden second primer whose
  effect on the eval is uncontrolled. The one-line output format
  preamble appended to the task statement is the minimum required.
- **Skip prompt caching to "match the cost reporting."** Rejected.
  Token counts are measured under `o200k_base`, independent of API
  billing; caching is purely a cost optimisation. Skipping it
  would multiply the eval cost by ~5x with no benefit.
- **Retry on bad output.** Rejected. See § Retry policy above —
  retrying on bad output is multi-sample evaluation under another
  name.
- **Use the Anthropic Python SDK's automatic retry loop** (which
  retries on a wider set of conditions). Rejected. Manual retry
  with a documented condition set is auditable in the metric file;
  the SDK's loop is a black box.

## Consequences

- **The Phase 3 gate is reproducible.** Same primer hash, same
  harness sha, same model id, same sampling → same numbers within
  the ±2-case tolerance. Stage 11's freeze ADR has clean evidence.
- **Stage 8 implementation is bounded.** One Anthropic Messages
  call per task, deterministic context construction, deterministic
  extraction. The metric writer is the largest component, scoped
  by [ADR 0055](0055-p3-metrics-schema.md).
- **The eval cost is predictable.** With prompt caching, the
  per-task API cost is dominated by the ~500-token output. A full
  60-task Sonnet run on `claude-sonnet-4-6` with caching is
  inexpensive enough that the ~6 runs of [phase-3-plan.md
  § Stage 9–10](../plans/phase-3-plan.md) fit a small budget.
- **CI is cheap.** `--dry-run` exercises the harness without paid
  API calls; only operator-triggered runs hit the API.
- **Reproducibility metadata is load-bearing.** Six months from now,
  reading a metric file that names its primer hash and harness sha
  unambiguously identifies what was measured. Without `run.json`
  the file is uninterpretable.

## Related decisions

- [ADR 0001](0001-target-tokenizer.md), [ADR 0051](0051-p3-tacit-token-rule.md)
  — the tokenizer-independent measurement plane that prompt caching
  doesn't touch.
- [ADR 0020](0020-sealing-held-out-in-repo.md) — sealed handling
  rules this ADR's sealed-task path follows.
- [ADR 0041](0041-p2-structured-error-format.md) — the diagnostic
  envelope captured in `failures/<task>/` for compile failures.
- [ADR 0050](0050-p3-primer-scope.md) — primer text the system
  field carries; the file's BLAKE3 hash is the
  `primer_blake3` field above.
- [ADR 0054](0054-p3-cross-family.md) — cross-family scope; uses
  this ADR's contract with provider = OpenRouter.
- [ADR 0055](0055-p3-metrics-schema.md) — metric file schema; this
  ADR pins what the run-metadata sidecar contains.
- [phase-3-plan.md § Stage 8, § Stage 9, § Risks](../plans/phase-3-plan.md)
  — implementation surface and flake-budget rule.
