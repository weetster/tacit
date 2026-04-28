# 0053 — Phase 3 maintenance / edit / repair sub-track scope

**Status:** Accepted
**Date:** 2026-04-28
**Phase:** 3, Stage 1
**Closes:** [phase-3-plan.md Q-P3-7](../plans/phase-3-plan.md);
[tacit-plan.md § Phase 3](../plans/tacit-plan.md) maintenance deferral

## Context

[tacit-plan.md § Phase 3](../plans/tacit-plan.md) lists a
maintenance / edit / repair sub-track as a Phase 3 deliverable but
defers scope: "task count, task shape, grading rubric, and gating
posture all open." [phase-3-plan.md § Stage 10](../plans/phase-3-plan.md)
asks Stage 1 to close those questions before Stage 10 implementation.

The primary gate measures *generation from a primer*. Real engineering
work is overwhelmingly editing existing code, not writing fresh code.
A primer-only generation gate is a clean falsification surface for the
language-density thesis but an incomplete signal for the AI-first-
language thesis: a language that is dense to write but resistant to
edit fails the working programmer's case. The maintenance sub-track
is the second axis.

The Q-P3-7 deferral has three concrete sub-questions:

1. **Task shape.** "Slightly larger programs than the corpus" was the
   parent plan's framing. The open corpus tasks average ~30–60
   authoring-view tokens of solution; a maintenance task wants
   ~100–200 tokens of starting material so an edit is meaningful.
2. **Task source.** Net-new authoring (expensive) versus derived
   (cheap, but limited to the corpus surface) versus hybrid.
3. **Grading rubric.** Edit and repair tasks succeed or fail on
   different axes than corpus tasks. Compile + test pass is
   necessary but not sufficient — a "repair" that rewrites the
   program from scratch passes the test but didn't *repair*
   anything.

The user's Stage 1 design pass settles on the hybrid approach: derive
repair tasks from the Stage 4–6 references (cheap, realistic), and
hand-author a small number of larger programs for edit and refactor
(net-new but bounded). Total ~12 tasks; not part of the Phase 3
go/no-go gate.

## Decision

**The maintenance sub-track ships ~12 tasks under
`corpus/maintenance/`, split between bug-injected repair tasks
derived from the Stage 4–6 Tacit-Lite references and hand-authored
edit/refactor tasks. Grading is per-task by compile-pass +
behavior-preservation + diff-locality. The sub-track is reported
under `plans/phase-3-results/` but is not part of the Phase 3
go/no-go gate.**

### Task scope

| Track   | Count | Source                                                              |
|---------|-------|---------------------------------------------------------------------|
| Repair  | 5–7   | Bug-injected variants of selected Stage 4–6 Tacit-Lite references.   |
| Edit    | 3–4   | Hand-authored programs ~100–200 tokens; small, well-scoped feature additions. |
| Refactor| 2–3   | Hand-authored programs ~150–250 tokens; behavior-preserving structural changes. |
| **Total** | **12**| **±2 acceptable; final count fixed at Stage 10 entry.**             |

### Layout

```
corpus/
  maintenance/
    README.md
    repair/
      r-001-<slug>/
        broken.tac
        broken.tac.sidecar.toml
        prompt.md
        tests.jsonl
        spec.toml
      r-002-<slug>/
      …
    edit/
      e-001-<slug>/
        starting.tac
        starting.tac.sidecar.toml
        prompt.md
        tests.jsonl
        spec.toml
      …
    refactor/
      f-001-<slug>/
        starting.tac
        starting.tac.sidecar.toml
        prompt.md
        tests-before.jsonl
        tests-after.jsonl
        spec.toml
      …
```

Per-task files:

- `broken.tac` / `starting.tac` — the program the model receives.
- `prompt.md` — the natural-language instruction to the model. For
  repair: "this program fails on input X with error Y, fix it." For
  edit: "extend this program to handle additional case Z." For
  refactor: "rewrite this program using <named idiom> while
  preserving behavior."
- `tests.jsonl` — test cases the model's output must pass. Same
  format as `corpus/tasks/<...>/tests.jsonl`. For refactor tasks,
  `tests-before.jsonl` and `tests-after.jsonl` are identical
  (behavior preservation); the harness asserts both pass on the
  output.
- `spec.toml` — harness-spec for grading: expected effect signature
  for the output's `main`, and any task-specific grading hints.

Ordering: ~5 repair, ~3 edit, ~3 refactor at the lower end of the
band; up to ~7 / 4 / 3 at the upper end.

### Repair task generation

Repair tasks are derived from Stage 4–6 Tacit-Lite references via
deliberate single-edit bug injection. The injected bugs are drawn
from a fixed catalogue:

| Bug class             | Example                                                |
|-----------------------|--------------------------------------------------------|
| off-by-one            | `@lt n 10` instead of `@le n 10`                       |
| swapped argument      | `@sub b a` instead of `@sub a b`                       |
| wrong primitive       | `@add` instead of `@mul`                               |
| missing recursive case| `lambda n. if n then ... else 0` body uses `n` not `n−1`|
| wrong effect annotation| `! {}` instead of `! {Mut}` on a function that mutates |
| primitive arity error | `@buf-copy dst 0 src 0` (4 args, expects 5)            |
| scope error           | reference to a name out of scope                       |

Each repair task picks **one** Stage 4–6 reference and injects
**one** bug from a different class than its sibling repair tasks
— the catalogue is the diversification surface. The
prompt names the symptom (test failure or compile error message)
without naming the bug class.

**Why repair tasks may use Stage 4–6 references as the substrate.**
[ADR 0049 § Why open Tacit references are off-limits to the
primer](0049-p3-examples-layout-contamination.md) forbids the
*primer* from drawing on `reference.tac` files. That rule protects
the *generation* gate, where the model writes Tacit-Lite from
scratch. Repair tasks measure a different capability: given a
broken Tacit-Lite program and an error symptom, can the model
produce a fix? The model does not need to have seen the original
correct version to repair it; the broken version is in the user
message, the primer teaches the language, and the test cases
verify behavior. The contamination concern from ADR 0049 doesn't
apply here.

**However**, repair tasks must not be drawn from corpus tasks that
also appear in the **primary generation track for the same eval
run.** Otherwise the model has seen the broken-fixed pair in the
primer's progressive examples and the repair becomes a recall
exercise. The Stage 1 ADR set picks repair-task seed references
from the *open corpus* (which the primer does not draw verbatim
from per ADR 0049) and never from `examples/phase-3/` (which the
primer does draw from).

### Edit task design

Edit tasks present the model with a working ~100–200-token Tacit-Lite
program and ask for a feature addition. Examples (working set, final
list pinned at Stage 10):

- A program that sums positive integers; edit asks for "also count
  the count of inputs and print both."
- A program that prints lines longer than 10 chars; edit asks for
  "also print a line count summary."
- A bounded-buffer echo program; edit asks for "skip blank lines."

Edit tasks are hand-authored under [ADR 0048](0048-p3-tacit-idiom-rules.md)
discipline. The starting program is **not** drawn from the open
corpus or from `examples/phase-3/` — those are eval material under
the primary track or primer source under ADR 0049. Edit tasks live
in their own untouched substrate.

### Refactor task design

Refactor tasks present a working program and a structural change
prompt: "rewrite this using `match` instead of nested `if`,"
"factor the inner loop into a named function," "switch from
hand-rolled buffer scanning to `@scan-byte`." Behavior preservation
is the load-bearing assertion; both `tests-before.jsonl` and
`tests-after.jsonl` (identical) must pass.

Substrate rule is the same as edit: hand-authored, not drawn from
corpus or `examples/phase-3/`.

### Grading rubric

For each task, the harness computes four numbers:

| Metric                | Definition                                                            |
|-----------------------|-----------------------------------------------------------------------|
| `compile_pass`        | The model's output Tacit-Lite compiles end-to-end.                    |
| `typecheck_pass`      | Output typechecks with the `spec.toml` effect signature.              |
| `tests_pass`          | All test cases in `tests.jsonl` (or `-before` and `-after`) pass.     |
| `diff_locality`       | Token-edit-distance between starting program and model output, normalised by starting program length. |

`diff_locality` is the maintenance-track-specific signal:

- For **repair** tasks, `diff_locality` should be small — a
  single-edit bug should be fixed by a single-edit patch. A
  rewrite-from-scratch passes `tests_pass` but fails the
  maintenance signal. The harness uses tiktoken `o200k_base`
  edit-distance, normalised by the starting program's token
  count. Threshold: **≤ 30%** of starting tokens edited counts as
  "local"; > 30% is "rewritten."
- For **edit** tasks, `diff_locality` is reported but not
  thresholded. A feature addition naturally edits more text than
  a bug fix; the metric is a data point, not a gate.
- For **refactor** tasks, `diff_locality` is **expected to be
  large** — a refactor changes structure. Reported, not
  thresholded.

A task's overall pass / fail is `compile_pass && typecheck_pass &&
tests_pass`, with `diff_locality` reported alongside.

### Sub-track aggregate

`corpus-eval --track maintenance` reports:

- Per-task: the four metrics above + token counts (input prompt +
  primer + generation).
- Per-track aggregates: pass rate over repair, edit, refactor
  separately and combined.
- Comparison to primary track: the same model under the same primer
  is presumed to have already run the primary track; the
  maintenance report cross-references that run's `run.json` in its
  own metadata.

### Reported, not gating

Per [phase-3-plan.md § Exit criteria](../plans/phase-3-plan.md), the
maintenance sub-track is **reported alongside but not part of the
go/no-go decision.** Stage 11's freeze ADR cites maintenance numbers
as evidence; it does not gate freeze on them. A bad maintenance
result on a passing primary gate produces a Phase 4+ follow-up
("primer revision for repair-style prompts") rather than a Phase 3
fail.

### Cross-family applicability

The maintenance tasks under `corpus/maintenance/` are run on every
model that runs the primary corpus, including the cross-family set
under [ADR 0054](0054-p3-cross-family.md). The harness has no
maintenance-vs-primary track special-casing per model; every model
sees the same task set under the same prompt format.

### CI integration

CI runs `corpus-eval --track maintenance --dry-run` per push, same
shape as the primary `--dry-run` from
[ADR 0052](0052-p3-eval-model-contract.md). Real runs are
operator-triggered.

## Alternatives considered

- **Net-new authoring of all 12 tasks.** Rejected on cost. The
  Stage 4–6 references are a free substrate for repair; using them
  reduces the new authoring burden from 12 tasks to ~5–7 tasks.
- **All-derived: repair tasks derived from corpus, edit/refactor
  tasks derived from corpus too.** Rejected. The corpus tasks are
  too small (~30–60 tokens) for meaningful edits and refactors.
  The substrate has to be larger.
- **Skip the maintenance sub-track entirely.** Rejected. The parent
  plan lists it as a Phase 3 deliverable; skipping is a scope
  reduction that needs its own ADR. Within the Q-P3-7 scope, the
  shippable minimum is what this ADR pins.
- **Gate Phase 3 on maintenance results too.** Rejected. The parent
  plan's "reported but not gating" stance is deliberate — primer-
  only generation is the falsification surface; maintenance is a
  second-axis measurement. Gating on both axes risks a coherent-on-
  one-axis fail being recorded as an outright fail.
- **Use larger token budget per maintenance task** (~500 tokens of
  starting material). Rejected. Larger starting material costs more
  authoring time and more eval cost without a clear reason; the
  ~100–250 token range is enough to produce a meaningful edit
  prompt without inflating costs.
- **Multi-edit repair tasks** (inject 2–3 bugs, ask the model to
  find and fix all). Rejected as too noisy. The single-edit
  discipline gives a clean diff-locality signal; multi-edit
  collapses the metric.
- **Allow LLM-assisted authorship of edit/refactor tasks.**
  Rejected for the same reason as ADR 0048 § Authorship: LLM-
  authored Tacit-Lite leaks into the eval surface as model-
  authored substrate, contaminating cross-family signals. Hand-
  authored or nothing.
- **Run maintenance only on Sonnet.** Rejected. Cost-saving but
  loses the cross-family comparison signal — running maintenance
  on every model that runs the primary track gives a per-family
  edit-fluency reading at marginal cost.

## Consequences

- **Stage 10 has a concrete brief.** ~12 tasks, three sub-tracks,
  per-task layout fixed, grading rubric mechanical.
- **Stage 4–6 references serve double duty.** They are eval-half
  material on the primary track and substrate for repair tasks on
  the maintenance track. The duty separation is enforced by
  prompt format (repair prompts present `broken.tac`, never the
  un-broken reference).
- **Edit and refactor authoring is bounded.** ~5–7 hand-authored
  programs total; substantial but not blocking.
- **The maintenance signal is per-axis.** Repair fluency, edit
  fluency, and refactor fluency report separately. A model that is
  good at one and bad at another shows up clearly in the metric.
- **`diff_locality` is a Phase-3-specific metric.** Token-edit-
  distance reporting is small additional harness work in
  Stage 10; the implementation is a standard token-level
  Levenshtein.
- **The sub-track is "reported, not gating."** A maintenance fail
  on a primary pass produces a primer-revision follow-up, not a
  Phase 3 fail. Stage 11's freeze ADR documents both numbers
  cleanly.
- **This ADR freezes with Stage 1.** Task counts, layout, and
  grading rubric are fixed; the actual task list is pinned at
  Stage 10 entry.

## Related decisions

- [ADR 0019](0019-corpus-idiom-rules.md), [ADR 0048](0048-p3-tacit-idiom-rules.md)
  — idiom rules; edit and refactor task starting material
  follows ADR 0048.
- [ADR 0049](0049-p3-examples-layout-contamination.md) — primer
  contamination boundary; repair-task substrate selection respects
  it.
- [ADR 0050](0050-p3-primer-scope.md) — primer; same primer used
  on the maintenance sub-track.
- [ADR 0052](0052-p3-eval-model-contract.md) — model contract;
  `--track maintenance` reuses it.
- [ADR 0054](0054-p3-cross-family.md) — cross-family; runs the
  maintenance sub-track on the same model set as primary.
- [ADR 0055](0055-p3-metrics-schema.md) — metric schema; extends
  to per-track aggregates and `diff_locality`.
- [phase-3-plan.md § Stage 10, § Exit criteria](../plans/phase-3-plan.md)
  — implementation surface and reported-not-gating posture.
- [tacit-plan.md § Phase 3](../plans/tacit-plan.md) — parent
  deferral this ADR closes.
