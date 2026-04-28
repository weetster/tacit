# 0051 — Tacit-Lite token-count rule for Phase 3

**Status:** Accepted
**Date:** 2026-04-28
**Phase:** 3, Stage 1
**Closes:** [phase-3-plan.md Q-P3-5](../plans/phase-3-plan.md)

## Context

[tacit-plan.md § Phase 3](../plans/tacit-plan.md) sets the
30%-reduction gate as "end-to-end token usage (primer + generation)
is ≥ 30% lower than equivalent Python." Two things must be pinned for
that arithmetic to be meaningful:

1. **What text is measured on the Tacit-Lite side.** Tacit-Lite has
   three textual representations: authoring view (what an author or
   model writes), canonical text format (what is stored as bytes for
   hashing per [ADR 0013](0013-canonical-text-format-frozen.md)), and
   inspection view (display-only per
   [ADR 0015](0015-inspection-view-scope.md)).
2. **Which tokenizer is applied.** [ADR 0001](0001-target-tokenizer.md)
   freezes the project tokenizer as tiktoken `o200k_base`; this ADR
   ratifies that choice for Phase 3 and pins how it composes with
   the three-way reporting split from
   [ADR 0021](0021-corpus-stdlib-dominance-reporting.md).

The Python baseline is unambiguous: ADR 0019 measures
`reference.py` source bytes under `o200k_base`. The Tacit-Lite side
needs the same level of pinning.

The argument for canonical-form measurement: canonical bytes are the
hash-identity of a program, deterministic across authors, and free of
display-name noise. The argument for authoring-view measurement: the
*model under test* writes authoring view, the primer teaches authoring
view, and the parent plan says "primer + generation tokens" — both of
which are authoring text. Canonical form is never seen by the model in
the eval loop.

## Decision

**A "Tacit-Lite token" is a tiktoken `o200k_base` token applied to the
authoring-view source text. The Phase 3 30%-reduction gate compares
the Tacit-Lite authoring-view token count against the Python source
token count under the same tokenizer. The reporting split from
ADR 0021 (full / stdlib-dominated / non-stdlib-dominated) applies
unchanged.**

### Authoring view, not canonical form

- **What is measured:** the bytes of the authoring-view source file
  (e.g., `reference.tac`) as they would be sent to a model under
  test.
- **What is not measured:** canonical-form bytes, sidecar bytes,
  inspection-view bytes, comments (forbidden anyway by
  [ADR 0048 § Authoring view](0048-p3-tacit-idiom-rules.md)).
- **Whitespace counts.** The same tokenizer applies to the same
  bytes Python is measured under. Stripping whitespace would be
  unfair on the Tacit-Lite side relative to a Python reference
  formatted by `ruff format`.

### Sidecar exclusion

The sidecar (`reference.tac.sidecar.toml`) is not part of the
authoring-view source the model under test sees. It carries
display-name and `[types.<binding>]` metadata that the Phase 2
typechecker consumes; the model neither reads nor produces it.
Sidecar bytes are excluded from Phase 3 token counts.

This matches how `harness/src/tacit_corpus/count_tokens.py` already
treats Python sources: it reads `reference.py` only, not any
ancillary file.

### Per-task and aggregate reporting

Per [ADR 0021](0021-corpus-stdlib-dominance-reporting.md):

- **Three Tacit-Lite aggregates** are reported alongside the existing
  three Python aggregates:
  - **Full** — sum over all in-scope tasks.
  - **Stdlib-dominated** — sum over tasks tagged
    `stdlib_dominated = true` in `corpus/stdlib-dominance.toml`.
  - **Non-stdlib-dominated** — sum over tasks tagged
    `stdlib_dominated = false`.
- **Per-task numbers** are reported alongside aggregates so that a
  near-miss can be diagnosed at task granularity.
- **The 30% gate** is evaluated against both the full aggregate and
  the non-stdlib-dominated aggregate, per ADR 0021's pass condition.
  The stdlib-dominated aggregate is reported but not gated.

### Primer counts on the Tacit-Lite side

[ADR 0050](0050-p3-primer-scope.md) sizes the primer at ~10,500
tokens with a 12,000-token cap. Those tokens are **input tokens to
every eval generation** and therefore count toward the per-task
Tacit-Lite token cost in the Phase 3 gate's "primer + generation"
arithmetic. The harness's `corpus-eval` command (Stage 8) reports:

- **Primer tokens** — fixed across all tasks, once per run.
- **Generation tokens** — per-task, model output bytes under
  `o200k_base`.
- **Tacit-Lite per-task cost** — primer + generation.

The Python baseline has no primer; its per-task cost is just
`reference.py` source bytes under `o200k_base` per ADR 0019.

The 30% comparison is therefore:

```
1 - (sum_tasks (primer + tacit_generation_tokens))
    / (sum_tasks python_reference_tokens)
```

— where the `primer` term is added to *every* task's cost (so it
appears N times in the numerator for an N-task aggregate). This is
the parent plan's "primer + generation" reading and matches how the
model is actually invoked: each task's prompt includes the full
primer.

### Tokenizer pinning

`tiktoken` library version is pinned in `corpus/harness/pyproject.toml`
(`tiktoken>=0.8`). The encoding name `o200k_base` is the load-bearing
fact, not the library version. ADR 0001's reopener clauses still
apply: if the OpenAI Python SDK changes the `o200k_base` table or
deprecates it, the Phase 3 measurement uses whatever the pinned
library version provides — same baseline rule as ADR 0019 for
Python.

The Claude API tokenizer is not used. Per ADR 0001 § Reopeners, the
project's measurement tokenizer is independent of the model under
test's tokenizer. Sonnet and Haiku internally tokenise differently
from `o200k_base`; the Phase 3 gate measures program *length* in a
fixed unit, not model-internal cost.

### What `corpus-tokens` reports

The existing `corpus-tokens` harness command (per
[`corpus/harness/src/tacit_corpus/count_tokens.py`](../corpus/harness/src/tacit_corpus/count_tokens.py))
extends in Stage 4 to measure Tacit-Lite reference token counts
alongside Python and Rust. The output gains a `tacit` column per
task and three aggregate columns:

```
task                              python   tacit    delta
arithmetic/001-sum-to-n             58       42      -28%
strings/011-reverse-string          47       35      -26%
…
full aggregate                    2380     1670     -30%
stdlib-dominated aggregate         480      550      14%
non-stdlib-dominated aggregate    1900     1120     -41%
```

(Numbers illustrative.)

## Alternatives considered

- **Measure canonical-form bytes.** Rejected. The model under test
  produces authoring view, not canonical form; the primer teaches
  authoring view; the eval loop never sees canonical form. A
  canonical-form measurement would compare a deterministic
  representation against an idiomatic one, which is a category
  mismatch. Canonical bytes are also typically smaller (no
  whitespace, no surface sugar) so the gate would become trivially
  easier to clear in a way that doesn't reflect the parent plan's
  thesis.
- **Measure with the Claude tokenizer.** Rejected. ADR 0001 already
  considered and rejected this; the project's measurement
  tokenizer is independent of the model under test. The Phase 3
  thesis is "Tacit-Lite is denser than Python in a fixed unit," not
  "Tacit-Lite is cheaper to run on Claude than Python is."
- **Subtract the primer cost from the comparison numerator.**
  Rejected. The parent plan's gate is "primer + generation" —
  including the primer cost is what the gate explicitly says to
  measure. A primer-excluded comparison would make a 17K primer
  free, which contradicts the Q-P3-4 compactness discipline.
- **Amortise the primer cost over the corpus** (count it once,
  divide by N). Rejected. Each model invocation pays the full
  primer cost; an amortised count understates real eval cost. The
  parent plan's "end-to-end token usage" reading is per-task
  inclusive.
- **Include sidecar bytes in the Tacit-Lite count.** Rejected. The
  model under test never sees sidecar; counting it would inflate
  Tacit-Lite's measured cost without representing what the eval
  measures. Sidecar exists to make the typechecker work, not to
  hold semantic content the model produces.
- **Strip whitespace before counting Tacit.** Rejected. Whitespace
  is in scope on the Python side ([ADR 0019](0019-corpus-idiom-rules.md)
  pins `ruff format`); the Tacit count must be on the same footing.

## Consequences

- **The Phase 3 gate's denominator is fully specified.** Combined
  with ADR 0019's Python rule, ADR 0021's three-way split, and ADR
  0050's primer cap, every term in the 30% calculation is pinned.
- **`corpus-tokens` extension is small.** One additional source
  type per task (`reference.tac`), one tokenizer call per source.
  Three additional aggregate rows in the output.
- **Authoring view is load-bearing for token economy.** ADR 0048's
  "no comments, no display-name renames for token gaming" rules
  are exactly what protects the gate from author-side knob-turning;
  this ADR makes that protection load-bearing.
- **The primer cost is a real budget pressure.** Every primer token
  is paid 47 times (open) or 60 times (open + sealed) in the
  per-task aggregate. A 1,000-token primer overrun is 47,000–60,000
  tokens of eval cost. ADR 0050's compactness discipline is the
  load-bearing response.
- **This ADR freezes with Stage 1.** Changes to the rule require a
  new ADR; pre-freeze adjustments are expected during Stage 1.

## Related decisions

- [ADR 0001](0001-target-tokenizer.md) — tokenizer; this ADR
  ratifies its use for Phase 3.
- [ADR 0019](0019-corpus-idiom-rules.md) — Python token measurement
  this ADR mirrors.
- [ADR 0021](0021-corpus-stdlib-dominance-reporting.md) — three-way
  aggregate split applied to Tacit-Lite token counts.
- [ADR 0048 § Authoring view](0048-p3-tacit-idiom-rules.md) —
  no-comments / no-rename rules that protect the count.
- [ADR 0050](0050-p3-primer-scope.md) — primer cap measured under
  this rule.
- [phase-3-plan.md § Stage 4 exit gate, § Stage 9](../plans/phase-3-plan.md)
  — where the count is reported.
