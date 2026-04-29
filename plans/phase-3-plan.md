# Phase 3 Implementation Plan

**Status:** Draft 2026-04-28
**Parent:** [tacit-plan.md](tacit-plan.md)
**Predecessor:** [phase-2-plan.md](phase-2-plan.md) (frozen 2026-04-28 — [ADR 0046](../decisions/0046-p2-stage-5-frozen.md))

Phase 3 is the project's primary falsification surface. The Phase 1–2 work
proves that a content-addressed AST can carry a language all the way from
authoring view through type-and-effect checking to native code. Phase 3 asks
the next question: **does a model fluent in human-oriented languages actually
write Tacit-Lite competently from a primer alone, and does it do so with
fewer tokens than equivalent Python?**

Phase 3 owns four concerns:

1. **A primer document** of ~10–17K tokens that teaches Tacit-Lite to a model
   in-context.
2. **Tacit-Lite reference solutions** for the open subset of the Phase 0
   corpus, sufficient to validate that the language can express each task and
   to seed the primer with worked examples.
3. **An evaluation harness** that drives a model with the primer in context,
   compiles its output through the Phase 1–2 pipeline, runs the corpus tests,
   and measures token cost against the Python baseline.
4. **The Phase 2 carry-over deferral** rolled into Phase 3 by
   [ADR 0046 § 3](../decisions/0046-p2-stage-5-frozen.md):
   non-trivial programs (sorting, basic data structures, file I/O beyond
   `echo`) typecheck with correct effect annotations and compile.

Out of scope by parent plan: refinement types, capabilities, effect handlers,
user-defined effects, row polymorphism, self-hosting, Python transpilation,
fine-tuning, scratch syscall stdlib. These are all deferred to Phase 7+ and
must not be discussed in Phase 3 ADRs except to record the boundary.

## Deliverables (from parent plan § Phase 3)

- ~10–17K-token primer document (written in the authoring view) structured
  as: one-page semantic summary, progressive Python/Rust ↔ Tacit-Lite pairs,
  idiom catalog, effect-reasoning examples, negative examples with structured
  explanations, compiler error catalog with fix patterns.
- Evaluation harness that runs the Phase 0 task corpus end-to-end: drives a
  model with the primer in context, captures generated Tacit-Lite, compiles
  it, runs the test cases, grades pass/fail, measures token cost.
- End-to-end token measurement: primer + generation tokens compared against
  the Python reference per [ADR 0019](../decisions/0019-corpus-idiom-rules.md)
  and reported under the [ADR 0021](../decisions/0021-corpus-stdlib-dominance-reporting.md)
  full / stdlib-dominated / non-stdlib-dominated split.
- Baseline measurements: Sonnet and Haiku performance with primer alone on
  the open corpus and on the sealed held-out subset.
- **Maintenance/edit/repair sub-track** — small second evaluation track of
  edit, repair, and refactor tasks. Scope, task count, and grading rubric
  are open and resolved as Q-P3 items in Stage 1.
- **Cross-family evaluation sub-track** — primary corpus + maintenance run
  against at least one Claude, one GPT, and one strong open-weight model.
  Family selection, primer-portability rules, and grading details are open
  and resolved as Q-P3 items in Stage 1.

## Carried over from Phase 2

Per [ADR 0046](../decisions/0046-p2-stage-5-frozen.md) § 3, Phase 2's exit
criterion 2 (non-trivial programs) is deferred to Phase 3:

- One sorting algorithm typechecks with correct effects and runs.
- One linked-list-style data structure typechecks with correct effects and
  runs.
- One file-I/O program beyond `echo` typechecks with correct effects and
  runs.

These are hand-authored, live under `examples/phase-3/`, and follow the same
hand-authoring discipline as the Phase 1 smoke corpus
([ADR 0020](../decisions/0020-sealing-held-out-in-repo.md)) — they are
**never drawn from `corpus/`**, sealed or open.

## What already exists

- The Phase 0 evaluation corpus ([`corpus/`](../corpus/)): 60 tasks across
  arithmetic / strings / collections / algorithms / I/O, each with
  `task.md`, `tests.jsonl`, `reference.py`, `reference.rs`. 47 open, 13
  sealed. Sealed integrity enforced by `corpus-verify-sealed` in CI per
  [ADR 0020](../decisions/0020-sealing-held-out-in-repo.md).
- Idiom rules for Python / Rust references frozen by
  [ADR 0019](../decisions/0019-corpus-idiom-rules.md). Token-count baseline
  rules frozen by [ADR 0021](../decisions/0021-corpus-stdlib-dominance-reporting.md).
- Tokenizer choice frozen by [ADR 0001](../decisions/0001-target-tokenizer.md):
  tiktoken `o200k_base`.
- Phase 1–2 compile pipeline: `tacit compile foo.tac -o foo`, `tacit check
  foo.tac`, `tacit view`. Nine-program typed smoke corpus under
  [`examples/smoke/`](../examples/smoke/).
- The harness skeleton at [`corpus/harness/`](../corpus/harness/) — a
  uv-managed Python project with `corpus-run`, `corpus-tokens`,
  `corpus-verify-sealed`. Phase 3 extends this; it does not replace it.
- [`stdlib/libc-effects.toml`](../stdlib/libc-effects.toml) populated with
  `write`, `read`, `exit` (Phase 1 surface, frozen by
  [ADR 0025](../decisions/0025-phase-1-libc-surface.md)) and consumed by
  Phase 2's effect checker.

What does **not** exist yet:

- Any Tacit-Lite source for corpus tasks.
- Stdlib coverage for the operations the corpus needs beyond raw bytes
  (integer parsing, integer formatting, line splitting, dynamic-size
  buffers, etc.).
- A way to drive a model with the primer in context from the harness.
- A primer.

## Sequencing

The pattern follows Phase 2: spec freezes first, then implementation stages
each gated by an exit criterion. Stage 1 is sequencing-critical; Stages
2–11 may overlap once their respective Stage 1 ADRs land. Stages are sized
so each is a plausible single-session task for Sonnet; the few that are
larger (4, 5, 6, 7) are explicit batch stages and call out the natural
session split.

### Stage 1: Spec ADRs — primer, stdlib expansion, eval surface (~2–3 weeks)

ADRs only. No production code, no primer prose, no Tacit reference
solutions. Stage 1 closes every spec question that Stages 2–11 would
otherwise have to bikeshed mid-implementation. Each ADR is independent
and a separate session; the stage is "complete" when every Q-P3-N below
has an Accepted ADR.

Open questions, numbered to extend the parent-plan / phase-1-plan /
phase-2-plan `Q-PN-N` scheme:

- **Q-P3-1 — Stdlib expansion surface for corpus coverage.**
  *Resolved by [ADR 0047](../decisions/0047-p3-stdlib-expansion-surface.md).*
  Which primitive `@name` symbols (extending the
  [ADR 0028](../decisions/0028-phase-1-libc-call-surface.md) allowlist)
  and which `libc-effects.toml` entries (extending
  [ADR 0025](../decisions/0025-phase-1-libc-surface.md)) Phase 3 needs to
  cover the corpus. The minimum surface is whatever the open 47 tasks
  collectively require; the working hypothesis is integer parse / format,
  string slicing, dynamic buffer growth, and stdin slurp, but the ADR
  enumerates the actual list and pins effect signatures.
- **Q-P3-2 — Tacit-Lite reference-solution idiom rules.**
  *Resolved by [ADR 0048](../decisions/0048-p3-tacit-idiom-rules.md).*
  The Tacit-Lite analogue of
  [ADR 0019](../decisions/0019-corpus-idiom-rules.md). Pins
  the canonical style for hand-authored Tacit references: use of `rec`,
  closure-vs-named-fn defaults, primitive-vs-loop conventions,
  generics-vs-monomorphic defaults, and the relationship between
  authoring-view source and sidecar metadata. Settles whether references
  ship as `reference.tac` + `.tacd` sidecar in each task directory or as
  a single combined artifact.
- **Q-P3-3 — `examples/phase-3/` layout and discipline.**
  *Resolved by [ADR 0049](../decisions/0049-p3-examples-layout-contamination.md).*
  Confirms a separate directory for the Phase 2 carry-over programs
  (sorting, data structure, file I/O) and the relationship between that
  directory and `corpus/tasks/<id>/reference.tac`. Closes the
  contamination question: `examples/phase-3/` is hand-authored and may
  seed primer prose; corpus references are hand-authored but are **not**
  primer material for the open set (the entire open set is fair primer
  material per ADR 0019, but using a specific task verbatim defeats the
  eval — pin the boundary explicitly).
- **Q-P3-4 — Primer scope and structure.**
  *Resolved by [ADR 0050](../decisions/0050-p3-primer-scope.md).*
  Target token count (within the parent plan's 10–17K window), section
  list, length budget per section, authoring-view fluency target (Sonnet
  must reach the >70% gate; Haiku-class is a stretch). Settles the
  primer's filename and location
  (likely `plans/primer/tacit-lite-primer.md` or a sibling tree).
- **Q-P3-5 — Tacit-Lite token-count rule.**
  *Resolved by [ADR 0051](../decisions/0051-p3-tacit-token-rule.md).*
  What counts as a "Tacit-Lite token" for the Phase 3 30%-reduction
  gate. Authoring-view text under tiktoken `o200k_base` per
  [ADR 0001](../decisions/0001-target-tokenizer.md) is the obvious
  answer; the ADR commits the obvious answer or, if a different rule
  is needed, justifies and freezes it. Aligns with the ADR 0021 full /
  stdlib-dominated / non-stdlib-dominated split.
- **Q-P3-6 — Eval-harness model invocation contract.**
  *Resolved by [ADR 0052](../decisions/0052-p3-eval-model-contract.md).*
  Which model identifiers count as "Sonnet" / "Haiku" for the baseline
  gate (vendor + exact version), sampling parameters (temperature,
  max tokens, retry budget), context construction (primer + task
  statement only — no test cases, no `reference.py`), and how the
  harness records run metadata for reproducibility (model id, sampling
  params, seed if available, primer hash, harness git sha).
- **Q-P3-7 — Maintenance/edit/repair sub-track scope.**
  *Resolved by [ADR 0053](../decisions/0053-p3-maintenance-subtrack.md).*
  Closes the "open" deferral in
  [tacit-plan.md § Phase 3](tacit-plan.md). Defines task count
  (target ~10–15), task shape (slightly larger programs than the
  corpus, edit / repair / refactor prompts), grading rubric (compile
  success, behavior preservation, token cost of the edit), and whether
  the sub-track is part of the Phase 3 go/no-go gate (parent plan says
  "reported alongside but not part of the go/no-go decision" — this ADR
  ratifies that).
- **Q-P3-8 — Cross-family sub-track scope.**
  *Resolved by [ADR 0054](../decisions/0054-p3-cross-family.md).*
  Closes the second "open" deferral. Pins the model families covered
  (one Claude, one GPT, one strong open-weight; specific model ids),
  primer-portability rules (one shared primer vs. per-family variants),
  the metric set (compile success, test pass, end-to-end token cost,
  repair success after deliberate error injection, authoring-view
  round-trip stability), and the materially-regression threshold that
  re-opens the primer design vs. a Phase 3 fail.
- **Q-P3-9 — Phase 3 metrics JSON schema.**
  *Resolved by [ADR 0055](../decisions/0055-p3-metrics-schema.md).*
  A single JSON schema for the per-run metrics file emitted by the
  harness: per-task pass/fail, per-task token counts (primer / generation
  / Python baseline), aggregate pass rate, aggregate token deltas,
  model and harness metadata. Reuses the
  [ADR 0041](../decisions/0041-p2-structured-error-format.md)
  diagnostic envelope for failed-compile cases so the Phase 2 typecheck
  output flows through unchanged.

Exit gate: every Q-P3-N has an Accepted ADR; the canonical-format /
stdlib amendments from Q-P3-1 ship with conformance test vectors landed
under [`plans/test-vectors/`](test-vectors/) (or note in the ADR if no
new canonical syntax is required). A **Stage 1 freeze ADR** closes the
stage, mirroring [ADR 0044](../decisions/0044-p2-stage-1-frozen.md).

**Stage 1 ADR landed (2026-04-28):** Q-P3-1 → [ADR 0047](../decisions/0047-p3-stdlib-expansion-surface.md);
Q-P3-2 → [ADR 0048](../decisions/0048-p3-tacit-idiom-rules.md);
Q-P3-3 → [ADR 0049](../decisions/0049-p3-examples-layout-contamination.md);
Q-P3-4 → [ADR 0050](../decisions/0050-p3-primer-scope.md);
Q-P3-5 → [ADR 0051](../decisions/0051-p3-tacit-token-rule.md);
Q-P3-6 → [ADR 0052](../decisions/0052-p3-eval-model-contract.md);
Q-P3-7 → [ADR 0053](../decisions/0053-p3-maintenance-subtrack.md);
Q-P3-8 → [ADR 0054](../decisions/0054-p3-cross-family.md);
Q-P3-9 → [ADR 0055](../decisions/0055-p3-metrics-schema.md). Stage 1
frozen by [ADR 0056](../decisions/0056-p3-stage-1-frozen.md).

### Stage 2: Stdlib expansion implementation (~1 week)

Implements the surface decided in Q-P3-1. Greenfield work in the existing
crates; no new crate.

- New `@name` primitive lowerings in `tacit-codegen` extending the
  [ADR 0028](../decisions/0028-phase-1-libc-call-surface.md) allowlist.
- New entries in `stdlib/libc-effects.toml` per the Q-P3-1 ADR. Schema
  extensions, if any, supersede [ADR 0025](../decisions/0025-phase-1-libc-surface.md)
  with a new ADR — never edit the schema in place.
- `tacit-typecheck` consumes the new entries via the existing
  libc-effects path; no typechecker code changes beyond the toml load.
- A small primitives-test corpus under `examples/smoke/` or
  `crates/tacit-codegen/tests/` that exercises every new primitive
  end-to-end with a typed signature.

Exit gate: every Q-P3-1 primitive lowers, links, runs, and typechecks
under `cargo test --features tacit-codegen/llvm19-1,tacit-cli/llvm19-1`;
the architecture doc at
[`docs/compiler-architecture.md`](../docs/compiler-architecture.md) gains
a Phase 3 codegen-additions table; clippy clean.

**Stage 2 landed (2026-04-28):** All eight primitives implemented in
`tacit-codegen` and `tacit-typecheck`; 14 conformance tests pass in
`crates/tacit-codegen/tests/p3_primitives.rs`; 14 source programs under
`examples/smoke/p3-*.tac`; `docs/compiler-architecture.md` Phase 3 table
added; clippy clean; `libc-effects.toml` unchanged (no new OS-boundary
symbols per ADR 0025/0047).

### Stage 3: Phase 2 carry-over programs (~3–5 days) ✓ LANDED 2026-04-28

Closes the deferral from [ADR 0046](../decisions/0046-p2-stage-5-frozen.md)
§ 3. Hand-authored Tacit-Lite programs under `examples/phase-3/`:

- **One sorting algorithm.** Concrete pick during this stage; insertion
  sort or selection sort over an `i64` buffer is the working assumption
  (quicksort is fine if Stage 2's stdlib supports it cleanly). Effect
  set: `Mut` for in-place; pure with explicit return otherwise.
- **One linked-list data structure.** Append + length, or cons + sum, or
  similar. Demonstrates a recursive type via the Phase 2 `forall` and
  `fn-ty` ADRs.
- **One file-I/O program beyond `echo`.** A working assumption:
  `sum-numbers` (read stdin lines, parse each as `i64`, sum, print).
  Effect set: `IO`. Does not duplicate any open corpus task verbatim.

Each program ships with its `.tac` source, sidecar, and a CI-runnable
end-to-end test alongside the existing smoke corpus.

Exit gate: all three programs compile, typecheck (with verified effect
signatures), link, and produce expected stdout / exit code under CI;
[`examples/phase-3/README.md`](../examples/phase-3/) lists them with
their effect signatures; Phase 2 exit criterion 2 is now satisfied.

### Stage 4: Tacit-Lite references — arithmetic + strings (corpus 001–020) (~1 week, ~2 sessions)

Hand-authored Tacit-Lite reference solutions for the open arithmetic
(001, 003, 004, 005, 006, 007, 009, 010 — eight tasks) and strings (011,
012, 013, 015, 016, 017, 018, 020 — eight tasks) categories. Sealed tasks
in these ranges (002, 008, 014, 019) get **no** Tacit reference; the
model writes those at eval time and the Python reference remains the
token-count baseline.

Each reference lives at the location pinned by Q-P3-2 (working
assumption: `corpus/tasks/<category>/<NNN-slug>/reference.tac` plus
sidecar). Each reference:

- Compiles end-to-end via `tacit compile`.
- Typechecks with effect signature matching the task's I/O contract.
- Passes every test case in the task's `tests.jsonl`.
- Follows the Q-P3-2 idiom rules (no code golf, no idiom-shopping).

A new harness command `corpus-run-tacit` runs the existing test cases
against the compiled Tacit reference for each task that has one. Lives
alongside `corpus-run`; reuses `corpus-run`'s sandboxing.

Natural session split: arithmetic (8 tasks) in one session; strings (8
tasks) in a second.

Exit gate: 16 references implemented; `corpus-run-tacit` passes on every
one; `corpus-tokens` reports Tacit-Lite per-task and aggregate token
counts alongside Python; CI runs `corpus-run-tacit` on every push.

### Stage 5: Tacit-Lite references — collections + algorithms part 1 (021–040) (~1 week, ~2 sessions)

Same shape as Stage 4. Open tasks:

- Collections open (021, 023, 025, 026, 027, 028, 030 — seven tasks).
- Algorithms 031–040 open (031, 032, 033, 035, 036, 037, 038, 040 — eight
  tasks).

Sealed in this range (022, 024, 029, 034, 039) get no Tacit reference.

Natural session split: collections (7) in one session; algorithms 031–040
(8) in a second.

Exit gate: 15 additional references implemented and passing under
`corpus-run-tacit`. Cumulative: 31 of 47 open references done.

### Stage 6: Tacit-Lite references — algorithms part 2 + I/O (041–060) (~1 week, ~2 sessions)

Same shape. Open tasks:

- Algorithms 041–050 open (041, 042, 044, 045, 046, 047, 049, 050 — eight
  tasks).
- I/O 051–060 open (051, 052, 054, 055, 056, 057, 059, 060 — eight tasks).

Sealed in this range (043, 048, 053, 058) get no Tacit reference.

Natural session split: algorithms 041–050 (8) in one session; I/O (8) in
a second.

Exit gate: all 47 open references implemented and passing under
`corpus-run-tacit`; aggregate `corpus-tokens` numbers reported for the
full open set; any task that resists clean Tacit expression is documented
as a Q-P3-1 follow-up (a primitive gap) and triggers a Stage 2 patch
rather than relaxing the reference rules.

### Stage 7: Primer document — core (~3–5 days, ~2 sessions)

Authors the bulk of the primer in the location pinned by Q-P3-4. Two
natural sessions:

- **Session A — semantic summary, progressive examples, idiom catalog.**
  One-page semantic summary; ~10–15 progressive Python ↔ Tacit-Lite
  pairs drawn from `examples/smoke/`, `examples/phase-3/`, and the
  Stage 4–6 references (open subset only — sealed material is
  off-limits per [ADR 0020](../decisions/0020-sealing-held-out-in-repo.md));
  the canonical idiom catalogue (lambda forms, `rec` shape, `match`
  shape, `let` shape, primitive call-sites).
- **Session B — effect reasoning + negative examples + error catalog.**
  Effect-propagation worked examples; effect-purity-violation worked
  examples; ~10 negative examples with the exact JSON diagnostic the
  compiler emits per [ADR 0041](../decisions/0041-p2-structured-error-format.md)
  and the fix; a compiler error catalog cross-referenced to the
  diagnostic envelope.

Exit gate: primer at the Q-P3-4 token target (within the parent plan's
10–17K window); every Tacit-Lite snippet in the primer compiles and
typechecks under a primer-fixture test (`cargo test
--features ...` extracts and validates each fenced block); no primer
example is drawn from `sealed/`.

### Stage 8: Evaluation harness — generation + grading (~1 week)

Extends `corpus/harness/` per Q-P3-6 and Q-P3-9:

- New entry point `corpus-eval` that:
  - Loads the primer and the per-task `task.md`.
  - Drives the configured model (Q-P3-6) with `(primer, task statement)`
    as context; captures the model's authoring-view output.
  - Writes the output to a temp directory; compiles it via the
    repo-local `tacit` binary; runs every test case in `tests.jsonl`
    against the resulting executable.
  - Records per-task metrics per the Q-P3-9 schema: compile pass,
    typecheck pass, test pass count, primer tokens, generation tokens,
    Python baseline tokens, full / stdlib-dominated / non-stdlib-dominated
    delta.
- Sealed handling: `corpus-eval` defaults to open-only and accepts
  `--include-sealed` for grading mode, mirroring `corpus-run`.
- Fail-mode capture: when the model emits code that doesn't compile or
  typecheck, the diagnostic envelope is captured under a
  `failures/<task>/` tree and counted against the pass rate; the primer
  error catalog from Stage 7 is the canonical fix reference.
- Reproducibility: every run writes a `run.json` with model id, sampling
  params, primer hash, harness git sha, corpus hash; same run id is the
  filename of the metrics output.

Exit gate: `uv run corpus-eval --model <id> --tasks 001` produces a
correct metrics record on a known-good model and primer; `--include-sealed`
gates verification correctly; CI runs `corpus-eval --dry-run` (compiles
the harness, validates the primer fixture, exercises the metrics writer
without invoking a paid model) on every push.

### Stage 9: Primary baseline run — Sonnet + Haiku (~2–3 days)

Runs `corpus-eval` end-to-end for the Phase 3 baseline gate.

- Sonnet on the open 47 tasks. Sonnet on the sealed 13 tasks under
  `--include-sealed`.
- Haiku on the same two sets.
- Two metric files committed under `plans/phase-3-results/` (or the
  Q-P3-9 location). Each carries its `run.json` and the
  per-family rollup.
- A short results note in `plans/phase-3-results/README.md` summarising
  the four runs, the aggregate pass rate, the aggregate token deltas
  (full / stdlib-dominated / non-stdlib-dominated per ADR 0021), and
  whether the parent-plan gate (Sonnet > 70% pass, ≥ 30% token
  reduction) is met.

This stage is mechanical — no design decisions left. If pass rates fall
short, the response is a Stage 7 primer revision and a re-run, not a
relaxation of the gate. If the gate is met, the data feeds directly
into Stage 11's freeze ADR.

Exit gate: four metric files committed; the README states the numbers;
the metric files validate against the Q-P3-9 JSON schema.

### Stage 10: Maintenance + cross-family sub-tracks (~1 week)

Per Q-P3-7 and Q-P3-8. Two parallel deliverables:

- **Maintenance sub-track.** Implement the ~10–15 edit / repair /
  refactor tasks in the format Q-P3-7 pins, under
  `corpus/maintenance/` (or the Q-P3-7 location). Add a `corpus-eval
  --track maintenance` mode that drives the model with the maintenance
  prompt format. Run on Sonnet and Haiku; record metrics.
- **Cross-family runs.** Run the primary corpus and the maintenance
  sub-track against the cross-family model set chosen in Q-P3-8 (one
  Claude beyond Sonnet/Haiku, one GPT, one open-weight). Record per-family
  metrics. The harness changes are minimal — `corpus-eval` is already
  parameterised by model id from Stage 8.

Both deliverables write into `plans/phase-3-results/` alongside Stage 9's
output. Per parent plan, neither sub-track gates the Phase 3 freeze; both
are reported.

Exit gate: maintenance metrics for at least Sonnet committed;
cross-family metrics for the Q-P3-8 model set committed; the results
README summarises both tracks; a material cross-family regression (per
the threshold pinned in Q-P3-8) is flagged as a primer follow-up rather
than a Phase 3 fail.

### Stage 11: Phase 3 freeze ADR (~1–2 days)

Closes Phase 3 in the same shape as
[ADR 0033](../decisions/0033-phase-1-frozen.md) and
[ADR 0046](../decisions/0046-p2-stage-5-frozen.md):

- Records what was built (primer, references for the open 47 tasks,
  carry-over programs, harness extensions, baseline + sub-track
  metrics).
- Records the parent-plan gate outcome: did Sonnet hit > 70% on the
  primary corpus, and did end-to-end token usage land ≥ 30% below the
  Python baseline on the full and non-stdlib-dominated aggregates?
- If the gate is met: Phase 3 frozen; Phase 4 (debugging tooling) is
  next; Phase 5 (synthetic training corpus) remains conditional and
  deferred per parent plan.
- If the gate is not met: Phase 3 is **not** frozen by this ADR; instead
  the ADR enumerates the gaps, opens a primer-revision cycle (back to
  Stage 7), and notes that Phase 5 may become urgent per parent plan
  § Phase 5.
- Updates [CLAUDE.md](../CLAUDE.md) current-phase annotation to "Phase 3
  complete; Phase 4 is next" (or, on a fail, "Phase 3 in primer-revision
  cycle").
- Records every Phase-3-specific deferral so a future phase can pick it
  up cleanly.

Exit gate: ADR Accepted; CLAUDE.md updated; results README points at the
ADR; `phase-3-plan.md` marked Frozen.

## Exit criteria

Per parent plan § Phase 3, in priority order:

1. **Primary gate (go/no-go).** Sonnet achieves > 70% pass rate on the
   open + sealed corpus using only the primer in context, **and**
   end-to-end token usage (primer + generation) is ≥ 30% lower than
   equivalent Python on both the full and non-stdlib-dominated
   aggregates per [ADR 0021](../decisions/0021-corpus-stdlib-dominance-reporting.md).
2. **Phase 2 carry-over.** All three non-trivial programs (sorting,
   data structure, file I/O) typecheck with correct effect annotations
   and compile under CI.
3. **Round-trip and inspection gates from Phases 1–2 hold.** No
   regression to the
   [ADR 0033](../decisions/0033-phase-1-frozen.md) authoring ↔
   canonical round-trip property, the L0/L1/L2 inspection-view
   fixtures, or the Phase 2 `--types`/`--effects` fixtures. Newly
   added stdlib primitives land with their own fixtures.
4. **Reported sub-tracks.** Maintenance and cross-family results are
   committed and readable, even on a primary-gate fail. A material
   cross-family regression is grounds to re-open primer design, not a
   Phase 3 fail.

Phase 4 must not begin until criteria 1–3 are met. Spec ambiguities
discovered during Phase 3 are bugs against either Phase 0–2 frozen
artifacts or the Stage 1 ADRs (per
[CLAUDE.md § Ground rules](../CLAUDE.md)) and are resolved with new
ADRs, not by relitigating frozen decisions.

## Risks

- **Primer design churn.** Phase 3's bet is that ~15K tokens of primer
  is enough. If Sonnet falls well short of the gate, the temptation is
  to grow the primer past the 17K cap to compensate. Mitigation: the
  Q-P3-4 token cap is binding; if it is not enough, that is a Phase 5
  signal (synthetic corpus + fine-tuning) per parent plan, not a primer
  bloat signal. Document and stop.
- **Stdlib creep.** Stage 2's primitive set is sized to cover the
  corpus, not the universe. If a Stage 4–6 reference asks for a
  primitive that is not in Q-P3-1, the response is a Q-P3-1
  follow-up ADR (extending the surface deliberately) and a Stage 2
  patch — not an in-line addition. Mirrors the Phase 2 risk register
  on `libc-effects.toml` schema drift.
- **Sealed contamination.** Stage 7's primer must not draw examples
  from `sealed/`. The `.claude/settings.json` denials and
  `corpus-verify-sealed` are guardrails, but the load-bearing rule is
  authorial discipline. Mitigation: the primer-fixture test in Stage 7
  cross-references every primer snippet against the open subset; any
  match against sealed content is a hard CI fail.
- **Idiom-shopping on Tacit references.** The Q-P3-2 idiom rules are
  the analogue of [ADR 0019](../decisions/0019-corpus-idiom-rules.md)
  for Tacit. Without them, Stage 4–6 authors have a knob that reaches
  past the 30% gate itself. Mitigation: ADR-frozen rules; references
  reviewed against the rules at PR time, not re-litigated.
- **Eval flakiness.** Sampling at temperature > 0 makes pass rates
  noisy. Mitigation: Q-P3-6 pins sampling parameters; Stage 9
  baseline runs report aggregate pass over the corpus, and any per-task
  flake within ±2 cases is acceptable; Stage 11 reads the aggregate,
  not per-task.
- **Cross-family primer portability.** The primer is authoring-view
  text; tokenizers differ across families. Mitigation: Q-P3-8 pins
  whether one shared primer or per-family primers are used; if a
  cross-family run reveals tokenizer pathology, that is a Q-P3-8
  re-open, not a Phase 3 fail.
- **Phase 2 carry-over expanding into a fourth program.** Stage 3's
  three programs are the contract from
  [ADR 0046](../decisions/0046-p2-stage-5-frozen.md). Adding a fourth
  ("while we're here, let's also do hash maps…") is a Phase 4 or later
  scope decision, not a Phase 3 line-item.
- **Effect-system creep redux.** Phase 3 must not extend the Phase 2
  effect lattice or generics. If a corpus task requires handlers, row
  polymorphism, or user-defined effects to express, the task is the
  wrong shape for Phase 3 — defer it (move to sealed-only, or to a
  Phase 7 follow-on corpus), do not extend the language.

## See also

- [tacit-plan.md § Phase 3](tacit-plan.md) — parent plan deliverable list.
- [tacit-plan.md § Phase 5](tacit-plan.md) — conditional follow-on if the
  primer-only bet fails.
- [phase-2-plan.md](phase-2-plan.md) — predecessor plan; the typed-and-
  effect-checked baseline Phase 3 builds on.
- [corpus/README.md](../corpus/README.md) — frozen Phase 0 evaluation
  corpus.
- [ADR 0001](../decisions/0001-target-tokenizer.md) — tokenizer.
- [ADR 0019](../decisions/0019-corpus-idiom-rules.md) — Python / Rust
  idiom rules; Q-P3-2 is the Tacit analogue.
- [ADR 0020](../decisions/0020-sealing-held-out-in-repo.md) — sealing
  mechanism; load-bearing for primer authorship.
- [ADR 0021](../decisions/0021-corpus-stdlib-dominance-reporting.md) —
  token-aggregate split that gates the 30% target.
- [ADR 0025](../decisions/0025-phase-1-libc-surface.md) — `libc-effects.toml`
  schema; Q-P3-1 extends additively.
- [ADR 0028](../decisions/0028-phase-1-libc-call-surface.md) — `@name`
  primitive surface; Q-P3-1 extends additively.
- [ADR 0033](../decisions/0033-phase-1-frozen.md),
  [ADR 0046](../decisions/0046-p2-stage-5-frozen.md) — phase-freeze ADR
  precedents that Stage 11 follows.
