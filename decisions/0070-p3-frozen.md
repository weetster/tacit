# 0070 — Phase 3 frozen

**Status:** Accepted
**Date:** 2026-05-06
**Phase:** 3 (exit)
**Closes:** [phase-3-plan.md § Stage 11](../plans/phase-3-plan.md)
**Supersedes:** None
**Artifacts frozen by this ADR:**
- [plans/primer/tacit-lite-primer.md](../plans/primer/tacit-lite-primer.md) — Tacit-Lite primer at 2,405-token stdlib appendix; rounds-1+2 surface; ADR 0050 § sections.
- [corpus/tasks/](../corpus/tasks/) — 47 open `reference.tac` solutions (sealed tasks unchanged).
- [corpus/tacit-reference-authorship.toml](../corpus/tacit-reference-authorship.toml) — ADR 0057 expert-authorship record.
- [corpus/harness/](../corpus/harness/) — `corpus-eval`, `corpus-run-tacit`, `corpus-tokens` extensions; repair-loop mode; `--include-sealed` and `--result-label` plumbing.
- [stdlib primitive surface] — the 34 `@name` primitives admitted across [ADR 0047](0047-p3-stdlib-expansion-surface.md) (Stage 1, 8 primitives) plus Bundles A–G ([ADR 0061](0061-p3-stdlib-bundle-a-i64-vectors.md), [0062](0062-p3-stdlib-bundle-b-text-indexing.md), [0063](0063-p3-stdlib-bundle-b2-token-index-any.md), [0064](0064-p3-stdlib-bundle-d-search-counting.md), [0065](0065-p3-stdlib-bundle-c-ordering-primitives.md), [0067](0067-p3-stdlib-bundle-e-stream-io-sugar.md), [0068](0068-p3-stdlib-bundle-f-ascii-classification.md), [0069](0069-p3-stdlib-bundle-g-utf8-codepoints.md)).
- [examples/phase-3/](../examples/phase-3/) — sorting, linked-list, and file-I/O carry-over programs (closes [ADR 0046](0046-p2-stage-5-frozen.md) § 3).
- [docs/phase-3-metrics.schema.json](../docs/phase-3-metrics.schema.json) — formal ADR 0055 metrics schema.
- [plans/phase-3-results/](../plans/phase-3-results/) — Stage 9 baseline runs, Stage 10 maintenance + cross-family runs, repair-loop summaries, and round-2 conclusion.
- [plans/phase-3-plan.md](../plans/phase-3-plan.md) — all eleven stages marked done.
- All prior Phase 3 artifacts frozen by ADRs 0056–0069 remain frozen.

## Context

Phase 3 was scoped in [phase-3-plan.md](../plans/phase-3-plan.md) as the
project's primary falsification surface. The phase asked one question and
two sub-questions:

> **Primary gate.** Does Sonnet, given only the primer in context, write
> Tacit-Lite competently for the corpus, *and* does the resulting primer +
> generation token cost land ≥ 30% under the equivalent Python baseline?

> **Carry-over.** Do the three Phase 2 non-trivial programs (sorting,
> linked-list, file I/O beyond `echo`) typecheck and run?

> **Reported sub-tracks.** What do the maintenance and cross-family
> sub-tracks look like under the same primer?

Stages 1–10 ran to completion. Stage 1 ([ADR 0056](0056-p3-stage-1-frozen.md))
closed the spec surface. Stages 2–6 implemented the stdlib expansion and
authored 47 open Tacit references. Stage 7 landed a 10,202-token primer.
Stage 8 wired `corpus-eval`. Stage 9 ran the Sonnet baseline. Stage 10
covered maintenance and cross-family. The library-mediated post-Stage-9
work added two stdlib rounds: round 1 ([phase-3-stdlib-next-steps.md](../plans/phase-3-stdlib-next-steps.md))
and round 2 ([phase-3-stdlib-round-2.md](../plans/phase-3-stdlib-round-2.md)).

The Stage-9 outcome required a re-interpretation of the gate. That
re-interpretation is the load-bearing decision in this ADR.

## Outcome of the primary gate

**The primer-only one-shot gate is not met, and the token gate was
structurally miscalibrated from the start.** Concretely:

- Best standalone Sonnet one-shot run: **29/47** open tasks (61.7%); the
  > 70% gate would require ≥ 33/47. Continued primer growth regressed to
  24/47 by run 7. Recorded in
  [plans/phase-3-results/README.md](../plans/phase-3-results/README.md).
- Token gate (Python baseline): every recorded primer-only run misses by
  orders of magnitude. The full open repair-loop run pays 1,118,376 primer
  tokens across all model calls against a 4,584-token Python baseline.
- Token gate (re-baselined): the Python comparison is apples-to-oranges.
  See § "The Python-baseline miscalibration" immediately below — the gate
  was unwinnable for any disciplined static-compilation language on this
  corpus, including Rust itself.

### The Python-baseline miscalibration

[tacit-plan.md § Phase 3](../plans/tacit-plan.md) sets the density gate as
"≥ 30% lower than equivalent Python." [ADR 0019](0019-corpus-idiom-rules.md)
and [ADR 0021](0021-corpus-stdlib-dominance-reporting.md) lock the Python
baseline. The corpus also carries a Rust reference for every task per
ADR 0019, but the gate is Python-relative.

`uv run corpus-tokens` on the 47 open tasks (`o200k_base`, ADR 0001):

| Reference | Total | Ratio vs Python |
|---|---:|---:|
| Python | 4,584 | 1.00× |
| **Rust** | **7,064** | **1.54×** |
| Tacit (current `reference.tac`, n=47) | 20,661 | 4.51× |
| Tacit (`reference.stdlib.tac`, n=23 covered) | 6,901 vs Python 2,350 on subset | 2.94× on subset |

Two implications:

1. **Rust itself is +54% over Python on this corpus.** The 30%-below-Python
   target is unwinnable for any disciplined static-compilation language —
   not because of Tacit-Lite design choices, but because Python's
   `sorted()`, slicing, set/dict literals, and comprehensions encode whole
   algorithms in a few BPE tokens via runtime-rich, heap-allocated, GC'd
   evaluation. Tacit-Lite explicitly does not share that evaluation model
   ([ADR 0022](0022-pure-kernel-host-model.md): pure computational kernel,
   impurity in the host).
2. **Tacit vs Rust is 2.92× on the full corpus and ~2× on the
   stdlib-covered subset.** This is the apples-to-apples ratio. It is the
   honest baseline for any future density target.

The authoring view itself is BPE-optimized: [ADR 0003](0003-authoring-view-bpe-compact.md)
selected `bpe-compact` after measuring five candidate grammars at 1.00–1.78×
under tiktoken `cl100k_base` and Claude's tokenizer. The 4.5×-Python /
2.9×-Rust ratio is *not* a syntax-density problem; it is a runtime-model
gap (vs Python) and a language-shape gap (vs Rust).

The Phase 3 Python-relative gate is therefore not just "missed" — it was
miscalibrated. This ADR records both: the gate as written is not met, and
the gate as written was not measurable to begin with for a static-
compilation language. § "Decision" item 6 retires the Python-relative
target; § "Strategic direction" item 4 carries Rust-relative density
forward as a Phase 4+ aspiration, not a gate.

### Post-Stage-9 experiments

[ADR 0060](0060-p3-repair-loop-outcome.md) declined to freeze on the
one-shot primer-only Stage 9 result and ran two follow-up experiments
under the explicit `library-mediated` and repair-loop labels per
ADR 0060 and the post-Stage-9 plan:

**Repair-loop result (Sonnet, primer-only).** Run
`019de6ef-e75e-70d8-aa52-e98c4c577f7d`:
- 30/47 one-shot, 40/47 final (85.1%) after up to two repair turns.
- 10/17 initially failed tasks recovered (5/11 invalid; 5/6 behavioral).
- 1.53 average model calls per task.

**Library-mediated result (Sonnet, primer + Bundles A–D).** Run
`019df533-fc2a-7511-ad6f-ebdc653878ae`:
- 32/47 one-shot (68.1%), 46/47 final (97.9%) after repair.
- 14/15 recovery; 100% invalid recovery; 93.3% behavioral.
- 1.36 average model calls per task.
- Single residual failure: `arithmetic/009-divisors` (algorithmic outlier,
  not a surface gap).

**Round-2 stdlib (Bundles E/F/G — stream I/O, ASCII class, UTF-8).** Canary
run `019dfd4d-a1e7-7197-b54b-5b3ee6d9fcfe` met three correctness gates
(12/12 final, 100% invalid recovery, 1.08 avg calls) but failed the three
density gates (one-shot improvement +1 vs needed +2; token reduction −9.6%
vs needed −25%; strings ratio 6.09× vs needed ≤ 4.0×). The round-2 stop
rule fired and no full open run was attempted. See
[plans/phase-3-results/ROUND_2_SUMMARY.md](../plans/phase-3-results/ROUND_2_SUMMARY.md)
and [plans/phase-3-stdlib-round-2.md § Conclusion](../plans/phase-3-stdlib-round-2.md).

**Cross-family.** GPT-5.4 (primer-only, repair-loop) reaches 91.5% final;
Haiku and GPT-5.4-mini plateau near 45%; OpenRouter availability blocked the
open-weight axis. See
[plans/phase-3-results/REPAIR_LOOP_EVALUATION_SUMMARY.md](../plans/phase-3-results/REPAIR_LOOP_EVALUATION_SUMMARY.md).

**Carry-over.** All three programs in `examples/phase-3/` typecheck with
verified effect signatures, link, and produce the expected output under CI.
Phase 2 exit criterion 2 ([ADR 0046](0046-p2-stage-5-frozen.md) § 3) is
satisfied.

**Round-trip and inspection gates.** Phase 1's authoring ↔ canonical
round-trip property and the L0/L1/L2 inspection-view fixtures, and Phase 2's
`--types`/`--effects` fixtures, all hold across the Phase 3 stdlib additions.
No regressions.

## Why freeze now rather than re-open primer revision

[phase-3-plan.md § Stage 11](../plans/phase-3-plan.md) anticipates two
outcomes — gate met (freeze) or gate missed (primer-revision cycle, back
to Stage 7). The plan's missed-gate branch presupposes that the gate is
missing because the primer is wrong. Three pieces of evidence collected
*after* the plan was written rule that interpretation out:

1. **Primer growth has decreasing-then-negative returns.** Seven
   primer-only Sonnet runs at increasing primer sizes show 6.4% → 38.3% →
   46.8% → 53.2% → 61.7% → 61.7% → **51.1%** task pass at 16,194 primer
   tokens. The curve flattened at 6, regressed at 7, and the regression is
   inside the parent-plan 10–17K window. Further primer iteration is not a
   credible path to >70% one-shot.
2. **Round-2 stdlib expansion does not close the residual token gap.**
   Bundles E/F/G solved patterns 1–4 of the round-2 plan's identified
   five failure-source patterns; pattern 5 (multi-value packing under
   no-tuples / no-closures / no-fold) dominated and is structural, not
   surface. The expert-authored `reference.stdlib.tac` files prove the
   ceiling: even with all 11 round-2 primitives, `is-anagram` requires
   three rec-lambdas to maintain a 256-byte count table; `longest-word`
   needs a 6-parameter lambda to thread `(pos, wstart, wcps, bstart,
   bblen, bcps)`. These are language-shape costs primitives cannot pay
   down.
3. **The Python-relative density gate was structurally unwinnable.** The
   corpus's own Rust reference is +54% over Python (§ "The Python-baseline
   miscalibration"). No disciplined static-compilation language can hit
   30%-below-Python on this corpus; the gate measures runtime-model
   richness, not language design. Re-running with a primer revision
   would not move this needle.

Findings 1 and 2 are recorded as ADR 0060 and the round-2 stop-rule
outcome. Finding 3 is novel to this ADR and is the load-bearing
re-interpretation: a missed gate that was unmeasurable to begin with is
not a "primer is wrong" verdict, regardless of finding 1.

Together they answer the gate question: the primer-only density
hypothesis (Python-relative) is **falsified-as-asked and unanswerable-as-
calibrated**, not under-iterated.

A "Phase 3 fail, re-open Stage 7" verdict treats the experiment as
unfinished. The empirical evidence shows the experiment is finished, with
a clear negative answer on Python-relative density (and a re-baselined
2.9× ratio against Rust) and a clear positive answer on
correctness-with-feedback. Freezing on those answers is the correct
disposition.

## Decision

**Phase 3 is frozen.** The freeze records:

1. **Primary gate verdict: not met, with two causes.** (a) The
   correctness-side gate (Sonnet ≥ 70% one-shot) is missed: best 61.7%,
   regressing under primer growth. The cause is structural (no tuples /
   records, no closures, no higher-order combinators), not surface
   (stdlib width or primer prose). (b) The density-side gate (≥ 30%
   below Python) was structurally miscalibrated: Rust on the same corpus
   is +54% over Python; no disciplined static-compilation language can
   meet a Python-relative density target on this corpus. The honest
   apples-to-apples baseline is Rust (Tacit currently 2.92× full corpus,
   ~2× on stdlib-covered subset). § "Empirical findings" records the
   data; § "Strategic direction" carries Rust-relative density forward
   as a Phase 4+ aspiration.

2. **Repair-loop and library-mediated results are accepted as primary
   evidence for Phase 4 direction.** Per ADR 0060, they remain reported
   under their own labels, not folded into the primer-only gate. Both
   exceed 90% final pass on Sonnet at < 1.6 avg model calls per task.
   Frontier-model fluency on Tacit-Lite under feedback is established.
   Python-relative density parity is structurally unreachable (and
   retired); Rust-relative density (currently 2.92×) is open and
   addressable through Phase 4+ language-shape work.

3. **Carry-over criterion 2 is satisfied.** Sorting, linked-list, and
   file-I/O programs under `examples/phase-3/` typecheck with effect
   signatures, link, and pass under CI. The Phase 2 carry-over from
   ADR 0046 § 3 closes here.

4. **Round-trip and inspection gates from Phases 1–2 hold.** No
   regression. New stdlib primitives ship with their own codegen and
   typecheck fixtures (`crates/tacit-codegen/tests/p3_primitives.rs`,
   `crates/tacit-typecheck/tests/stdlib_*.rs`).

5. **Maintenance and cross-family sub-tracks are reported.** Per parent
   plan, neither gates the freeze. A material cross-family regression is
   *not* present (GPT-5.4 91.5% repair, Sonnet 97.9% library-mediated;
   the open-weight axis is recorded as evaluation-blocked, not as a
   regression).

Concretely:

1. **The Tacit-Lite stdlib surface is locked to the 34 admitted `@name`
   primitives.** Stage 1 admits 8 (ADR 0047, PARSE/FORMAT/MEM/STACK-ALLOC).
   Bundles A/B/B2/C/D add 15. Bundles E/F/G add 11. The full distribution
   is in § "Stdlib primitive count" below. Further primitives are explicit
   Phase 4+ work and require a new ADR; bug fixes to existing primitives
   do not.

2. **The primer is locked at the Stage 7 + round-2 form.** Total primer
   measures 12,607 `o200k_base` tokens (10,202 core + 2,405 stdlib
   appendix). Further primer changes that affect the gate calculation
   require a new ADR.

3. **The corpus references are locked.** All 47 open `reference.tac` and
   the 12 round-2 `reference.stdlib.tac` files are the Phase 3 authoring
   record. Sealed tasks remain free of Tacit references per ADR 0049.
   Future re-authoring is a new-ADR decision.

4. **The harness CLI surface is locked.** `corpus-eval`, `corpus-run-tacit`,
   `corpus-tokens`, the `--track`, `--include-sealed`, `--result-label`,
   and repair-loop flags, and the run-id-keyed `<id>.run.json` /
   `<id>.metrics.json` outputs are normative. Further harness changes that
   affect metrics shape require a new ADR; bug fixes do not.

5. **The metrics schema is locked.** `docs/phase-3-metrics.schema.json`
   is the normative output contract per ADR 0055 (amended for repair-loop
   accounting by ADR 0060). The four-atom diagnostic-kind extension
   (`test-failure`) per ADR 0056 § 5 stays inside metrics; it is **not**
   emitted by `tacit-typecheck`.

6. **The Python-relative 30%-reduction density target is retired.** It
   was structurally unwinnable for any disciplined static-compilation
   language on this corpus (Rust loses to it by +54%). It is *not*
   replaced by an equivalent Rust-relative gate in this freeze; future
   density work tracks against Rust as an *aspiration*, not a gate, and
   any binding density target in Phase 4+ requires a new ADR. The
   `corpus-tokens` harness already reports per-task and aggregate Rust
   tokens (ADR 0019, ADR 0021) and continues to do so.

7. **Phase 4 may begin.** Phase 4's first act is a `phase-4-plan.md`
   scoping language-shape work (tuples / records, closures, higher-order
   combinators) and debugging tooling. Phase 4 may set a Rust-relative
   density aspiration in its plan (e.g., "≤ 1.5× Rust on the corpus")
   but is not required to gate on density. Phase 4 may not relitigate
   Python parity as a target — that is closed by item 6 and cannot be
   reopened without new evidence beyond what Round 2 produced. The
   structural-positioning pivot in § "Strategic direction" is binding
   on Phase 4 scope: language-shape work justified primarily as
   "reasoning support" rather than "density chase."

## Empirical findings

### Round-by-round summary

| Track | Best one-shot | Final (with repair) | Avg calls | Token delta vs Python |
|-------|---:|---:|---:|---:|
| Sonnet primer-only (best of 7) | 29/47 (61.7%) | n/a | 1.0 | +14,491.8% |
| Sonnet primer-only (run 7, regressed) | 24/47 (51.1%) | n/a | 1.0 | +16,975.9% |
| Sonnet primer-only repair-loop | 30/47 (63.8%) | 40/47 (85.1%) | 1.53 | +25,220.5% |
| Sonnet library-mediated (rounds 1–2 surface) | 32/47 (68.1%) | 46/47 (97.9%) | 1.36 | reported library-mediated |
| GPT-5.4 primer-only repair-loop | 28/47 (59.6%) | 43/47 (91.5%) | 1.51 | +30,260% |
| Haiku primer-only repair-loop | 10/47 (21.3%) | 22/47 (46.8%) | 2.34 | +46,900% |
| GPT-5.4-mini primer-only repair-loop | 10/47 (21.3%) | 21/47 (44.7%) | 2.45 | +49,050% |

Sources: [plans/phase-3-results/README.md](../plans/phase-3-results/README.md),
[plans/phase-3-results/REPAIR_LOOP_EVALUATION_SUMMARY.md](../plans/phase-3-results/REPAIR_LOOP_EVALUATION_SUMMARY.md),
ADR 0060.

The "Token delta vs Python" column is the gate-as-written denominator and
is the apples-to-oranges comparison documented in § "The Python-baseline
miscalibration" above. The honest static-vs-static figure is Tacit
**2.92× Rust** on the full open corpus (20,661 vs 7,064), or
**~2× Rust** on the 23-task stdlib-covered subset. Phase 4 density
tracking uses the Rust denominator.

### What the round-2 canary settled

Round 2 was framed as a single, narrow falsification: can byte-level surface
expansion (Bundles E/F/G — `@stdin-slurp`, `@write-range`, `@buf-rev`,
`@ascii-tolower`, `@ascii-toupper`, `@ascii-is-alpha`, `@ascii-is-digit`,
`@ascii-is-space`, `@utf8-decode`, `@utf8-encode`, `@utf8-len`) close the
strings/IO token gap to ≤ 3.5× Python? The answer is **no**, with cause:

| Plan-§ pattern | Surface gap or language-shape | Round-2 verdict |
|---|---|---|
| 1. One-byte-at-a-time stdin loop | surface | solved (`@stdin-slurp`) |
| 2. Manual UTF-8 decode | surface | solved (`@utf8-decode`/`-encode`) |
| 3. Manual ASCII case shift | surface | solved (`@ascii-tolower`/`-upper`) |
| 4. Byte-class enumeration | surface | solved (`@ascii-is-*`) |
| 5. Packed-integer multi-return | language-shape | unchanged |

Per-task tokens *rose* on the heaviest strings tasks despite the new
primitives:
- `strings/020-is-anagram`: 550 → 773 (+40%).
- `strings/016-longest-word`: 294 → 596 (+103%).

The model used the new primitives but composed more carefully (more
correct), not more compactly. Expert-authored references prove the
ceiling, not a model limitation: even with all 11 primitives,
`is-anagram` requires three rec-lambdas to maintain a 256-byte count
table because Tacit-Lite has no tuples to pack the state, and
`longest-word` requires a 6-parameter lambda threading
`(pos, wstart, wcps, bstart, bblen, bcps)` because there is no record /
tuple type to package multi-value returns.

This is the *fifth* (and dominant) failure-source pattern from the round-2
plan, deferred at plan time as language-shape work. Round 2 confirms that
deferral was correct: byte-level surface cannot substitute for tuples or
higher-order combinators.

### What this falsifies and what it does not

**Falsified:**
- "Primer growth alone, within the 17K window, can lift one-shot pass to
  > 70%." (Negative returns by run 7.)
- "Stdlib expansion alone, in the absence of language-shape work, can
  close meaningful ground on per-task density." (Round 2 with all 11
  byte-level primitives still 6.09× Python on the strings canary;
  per-task Tacit tokens *rose* on the heaviest tasks.)
- "Tacit-Lite can approach Python on per-task density on this corpus."
  (Empirically refuted, but also structurally unwinnable: Rust on the
  same corpus is +54% over Python.)

**Unanswerable as calibrated** (and therefore retired, not refuted):
- "Tacit-Lite is ≥ 30% denser than Python on the corpus." The
  comparison is apples-to-oranges. Python's density advantage is
  runtime-model richness, not language-design merit. Re-running with a
  different primer or stdlib does not change this.

**Not falsified, and live for Phase 4+:**
- "Tacit-Lite can approach Rust density on the corpus with language-shape
  work." Tacit is currently 2.92× Rust full / ~2× on the stdlib-covered
  subset. Pattern-5 evidence (lambda threading, packed-arithmetic
  multi-return) suggests tuples + closures + combinators close
  meaningful ground. ≤ 1.5× Rust is plausible-with-effort, not gated.
- "Frontier models can write competent Tacit-Lite under compiler/test
  feedback." (Sonnet 97.9% library-mediated; GPT-5.4 91.5% primer-only.
  Both at < 1.6 avg model calls.)
- "Stdlib primitives compose without recipes." (Round 1 conclusion holds:
  Bundles A–D used without task-shaped primer prose.)
- "Round-trip / inspection gates survive a richer surface." (No
  regressions; new fixtures land alongside primitives.)
- "Effect signatures track byte-level I/O cleanly." (Bundles E/F/G use
  `IO` / `Mut` / `{}` from the existing four-atom lattice; no extension
  required.)

## Strategic direction (option 3, refined)

The empirical evidence converges on the strategic choice from
[plans/phase-3-results/FREEZE_ADR_EXCERPT.md](../plans/phase-3-results/FREEZE_ADR_EXCERPT.md),
adopted here with one clarification:

> **Lead positioning with structure, not density-vs-Python.** Tacit's
> advantage is not token count per task against a runtime-rich dynamic
> language, but reasoning support: content-addressed code, effect-tracked
> correctness, and language-shaped program structure. Invest Phase 4 in
> features that make program reasoning *easier* (tuples, effect
> signatures, closures), not in features that chase Python token parity
> (which is structurally unwinnable for any static-compilation language
> on this corpus).

The clarification: density is not abandoned as a project concern. The
Python-relative target is retired (it was miscalibrated). Density vs
Rust — the apples-to-apples comparison — remains a tracked aspiration.
This is a softer pivot than "abandon density" and matches the data:
Tacit is 2.92× Rust today; language-shape work plausibly closes meaningful
ground.

Concrete consequences for Phase 4 scope:

1. **Round 3 stdlib expansion is not on the Phase 4 roadmap.** The
   per-bundle ROI curve has flattened. New primitives may still land
   for primitive *correctness* reasons (a clear semantic gap, a
   safety-relevant intrinsic) but not as a density play.
2. **Tuples / records are first-class Phase 4 candidates.** Pattern-5
   evidence shows them load-bearing for any further compaction.
3. **Closures and higher-order combinators are Phase 4 / Phase 7
   candidates.** They sit above tuples in dependency order; the
   `for-each` / `map` / `fold` shape is not expressible without a
   value-of-function story richer than today's closed-lambda ADR 0026.
4. **Density tracking re-baselines from Python to Rust.** The Python-
   relative 30%-reduction target is retired as a gate (it was
   structurally unwinnable). The corpus's Rust references — already
   reported by `corpus-tokens` per ADR 0019 / ADR 0021 — become the
   apples-to-apples reference. Phase 4 plans *may* set a
   Rust-relative aspiration (e.g., "≤ 1.5× Rust on the corpus" is a
   plausible target given the current 2.92× ratio and the structural
   savings tuples + combinators are expected to produce). They *must
   not* set Python-relative density gates without new evidence.
5. **Marketing / positioning.** Per the working stance recorded in
   `MEMORY.md`, the project's external framing leads with *structural*
   properties (content-addressed AST, BLAKE3 identity, DeBruijn
   indices, effect lattice, ADR discipline, LLVM-native) rather than
   audience claims. This ADR is the empirical foundation for that
   pivot. External-facing density claims, when made, should cite the
   Rust comparison, not the Python comparison; and should frame
   density as a Phase 4+ aspiration rather than a Phase 3 outcome.

This direction is reported, not pre-frozen — the Phase 4 plan ADR will
be the binding scope artifact. § "Decision" items 6–7 commit the
Python-target retirement and the Rust-aspiration permission; everything
else here is guidance for Phase 4 authorship.

## Stdlib primitive count

For the record, the locked Phase 3 stdlib surface is 34 `@name` primitives
across nine ADRs, distributed:

| Origin | Count | Primitives |
|---|---:|---|
| ADR 0047 (Stage 1) | 8 | `@parse-i64`, `@fmt-i64`, `@buf-get`, `@buf-set`, `@buf-copy`, `@buf-eq`, `@scan-byte`, `@buf-alloc-dyn` |
| ADR 0061 (Bundle A) | 4 | `@i64-get`, `@i64-set`, `@i64-swap`, `@i64-copy` |
| ADR 0062 (Bundle B) | 4 | `@line-index`, `@token-index`, `@range-start`, `@range-len` |
| ADR 0063 (Bundle B2) | 1 | `@token-index-any` |
| ADR 0065 (Bundle C) | 3 | `@sort-i64`, `@sort-ranges-by-bytes`, `@stable-sort-pairs-i64` |
| ADR 0064 (Bundle D) | 3 | `@lower-bound-i64`, `@count-equal-ranges`, `@dedup-adjacent-ranges` |
| ADR 0067 (Bundle E) | 3 | `@stdin-slurp`, `@write-range`, `@buf-rev` |
| ADR 0068 (Bundle F) | 5 | `@ascii-tolower`, `@ascii-toupper`, `@ascii-is-alpha`, `@ascii-is-digit`, `@ascii-is-space` |
| ADR 0069 (Bundle G) | 3 | `@utf8-decode`, `@utf8-encode`, `@utf8-len` |
| **Total** | **34** | |

## Deferred items

Items raised during Phase 3 and explicitly deferred:

1. **Tuples / records (pattern-5 fix).** The dominant remaining token
   cost. Phase 4 work; should be the first Phase 4 ADR.
2. **Closures over locally-bound state.** Required for `for-each`-shaped
   combinators. Phase 4 / Phase 7 boundary; depends on the
   closed-lambda model in ADR 0026 being either lifted or extended.
3. **Higher-order combinators (`for-each`, `map`, `fold`).** Phase 4+;
   blocked on closure semantics.
4. **General hash maps.** Already deferred to Phase 7 by
   ADR 0047 § "Acknowledged gaps" (`io/056-unique-lines`); freeze
   re-confirms the deferral.
5. **Module / import system for user-level libraries.** Documented as
   long-term in [phase-3-stdlib-next-steps.md § Surface Strategy](../plans/phase-3-stdlib-next-steps.md);
   not in Phase 4's first scope unless tuples + closures are blocked on
   it.
6. **Sealed-safe repair feedback policy.** ADR 0060 deferred this and
   no sealed repair run has been attempted. A policy-defining ADR is
   prerequisite to any sealed repair-loop run; until then `corpus-eval
   --include-sealed` remains valid for one-shot grading only.
7. **Open-weight cross-family axis.** Blocked on OpenRouter
   availability. The cross-family record under ADR 0066 is two of three
   axes (Anthropic, OpenAI). The open-weight axis is documented as
   evaluation-blocked, not as a regression.
8. **Round 3 stdlib expansion.** Explicitly *not* deferred — declined per
   round-2 conclusion and § "Strategic direction (option 3)".
9. **Phase 5 (synthetic training corpus + fine-tuning).** Parent plan
   makes this conditional on the primer-only outcome. The outcome is
   missed. Phase 5 *may* become urgent later; this ADR does not commit
   to it. Phase 4 (language-shape + tooling) precedes any Phase 5
   decision.

## Alternatives considered

- **Re-open Stage 7 primer-revision cycle per the plan's missed-gate
  branch.** Rejected. The empirical evidence (run-7 regression; round-2
  ceiling; pattern-5 expert-reference proof) shows further primer
  iteration cannot lift the gate. The plan was written before that
  evidence existed.

- **Run a third stdlib round.** Rejected. The round-2 stop rule fired on
  the canary; pattern 5 dominates the residual gap and is structural.
  Round 3 would invest tokens in primitives that cannot move the
  density curve.

- **Freeze without recording the option-3 pivot.** Rejected. Phase 4's
  scope decision depends on knowing the density target is retired; an
  ambiguous freeze invites re-litigating.

- **Re-baseline the gate to Rust-relative and re-run instead of
  freezing.** Rejected. The corpus-tokens output already shows Tacit
  is 2.92× Rust full / ~2× Rust on the stdlib-covered subset; this is
  the same 2× shortfall the round-2 stop rule already analyzed,
  expressed against a different denominator. Re-running primer-only
  Sonnet against a Rust-relative 30%-reduction target would also miss,
  for the same pattern-5 reason. The right disposition is to record
  the empirical state (Phase 3 frozen, Python gate retired, Rust
  becomes the tracked aspiration) and let Phase 4 set its own target
  once tuples + closures are on the table — at which point the ratio
  will move and a fresh measurement is meaningful.

- **Replace the Python gate with an explicit Rust-relative gate in this
  freeze ADR.** Rejected. Tacit is 2.92× Rust today, with no Phase 4
  features landed; setting a hard target now (e.g., "≤ 1.5× Rust") in
  a freeze ADR pre-commits Phase 4 scope without the language-shape
  ADRs that would justify a specific target. § "Decision" item 7
  permits — but does not require — a Rust-relative aspiration in the
  Phase 4 plan. That is the right scope for setting one.

- **Mark Phase 3 a fail and stop the project.** Rejected. The
  density hypothesis is one of several Phase 3 hypotheses; the
  correctness-under-feedback hypothesis is supported, the carry-over
  criterion is met, and the round-trip / inspection gates hold. The
  project's structural advantages (ADR-gated design, content-addressed
  AST, four-atom effect lattice, frozen canonical form) are intact and
  load-bearing for Phase 4. A "fail and stop" reading conflates one
  failed hypothesis with the project's overall thesis.

- **Promote the library-mediated 97.9% Sonnet result to the primary
  gate.** Rejected per ADR 0060. Library-mediated runs add the stdlib
  surface to the workflow; they answer a different question. Reporting
  them under `--result-label library-mediated` is the correct
  disposition.

- **Promote the GPT-5.4 91.5% repair result to the primary gate.**
  Rejected. The primary gate names Sonnet as the evaluated model
  (ADR 0052). Cross-family is reported, not gating.

## Consequences

- **Phase 4 begins.** First action is `plans/phase-4-plan.md` scoping
  language-shape work (tuples / records, closures, combinators) and
  debugging tooling, against the structural-positioning pivot.
- **CLAUDE.md updated.** Current-phase annotation now reads "Phase 3
  complete; Phase 4 is next." The Phase 3 deferred items above carry
  forward.
- **CI is stable.** The CI matrix continues to exercise format, clippy
  (with LLVM), unit + integration tests, the nine-program Phase 1 +
  Phase 2 smoke corpus, the three Phase 3 carry-over programs, and
  `corpus-eval --dry-run` against the open Tacit references. No
  Phase-3-specific CI is removed; new CI for Phase 4 will land
  alongside its plan.
- **The corpus is preserved.** `corpus/` (tasks, harness, sealed
  guardrails, idiom rules, stdlib-dominance reporting, ADR 0057
  authorship record) all remain frozen as evaluation infrastructure.
  Phase 4 may *use* the corpus for regression but must not edit
  references except via a new ADR.
- **The Python-relative 30%-density target is retired.** It was
  structurally unwinnable for any disciplined static language on this
  corpus. Future ADRs may reference it as historical context; they may
  not invoke it as a gate or as a Phase 4 deliverable. Density work
  going forward tracks against Rust, not Python.
- **The marketing / positioning pivot is empirically grounded.** The
  working direction recorded in `MEMORY.md` (lead with structure, not
  audience) now has a Phase-3 ADR backing it. External-facing density
  claims, when made, cite the Rust comparison (Tacit 2.92× Rust on
  the open corpus) rather than Python. Future README / plan edits
  may align without further re-litigation.

## Related decisions

- [ADR 0001](0001-target-tokenizer.md) — tokenizer (`o200k_base`); the
  measurement frame for the density analysis here.
- [ADR 0003](0003-authoring-view-bpe-compact.md) — authoring-view
  grammar selected by BPE measurement against four alternatives. The
  4.51×-Python ratio is *not* a syntax inefficiency; this ADR confirms
  the authoring view is BPE-optimized.
- [ADR 0019](0019-corpus-idiom-rules.md) — Python and Rust idiom rules;
  the Rust references this ADR re-baselines against were authored under
  ADR 0019.
- [ADR 0021](0021-corpus-stdlib-dominance-reporting.md) — corpus
  three-way split. `corpus-tokens` already reports Rust; the Python-
  relative gate did not consume the Rust column.
- [ADR 0022](0022-pure-kernel-host-model.md) — pure-kernel-host model;
  the structural reason Tacit-Lite cannot match Python's runtime-rich
  density.
- [ADR 0033](0033-phase-1-frozen.md) — Phase 1 freeze; established the
  freeze-ADR discipline.
- [ADR 0046](0046-p2-stage-5-frozen.md) — Phase 2 freeze; deferred the
  carry-over closed by this ADR.
- [ADR 0056](0056-p3-stage-1-frozen.md) — Phase 3 Stage 1 freeze; locked
  the spec surface this ADR builds against.
- [ADR 0057](0057-p3-expert-agent-tacit-reference-authorship.md) —
  Tacit-reference authorship; load-bearing for the corpus references
  frozen here.
- [ADR 0058](0058-p3-closed-multi-arg-helper-lowering.md),
  [ADR 0059](0059-p3-rec-hidden-captures.md) — codegen amendments
  required by Stage 4–6 reference authorship.
- [ADR 0060](0060-p3-repair-loop-outcome.md) — repair-loop verdict and
  reporting framing; this ADR consumes it unchanged.
- [ADR 0061](0061-p3-stdlib-bundle-a-i64-vectors.md) –
  [ADR 0065](0065-p3-stdlib-bundle-c-ordering-primitives.md) — round-1
  stdlib bundles.
- [ADR 0066](0066-p3-cross-family-tier-matching.md) — cross-family tier
  matching; open-weight axis blocked by OpenRouter availability.
- [ADR 0067](0067-p3-stdlib-bundle-e-stream-io-sugar.md),
  [ADR 0068](0068-p3-stdlib-bundle-f-ascii-classification.md),
  [ADR 0069](0069-p3-stdlib-bundle-g-utf8-codepoints.md) — round-2
  stdlib bundles; the surface this ADR locks.
- [phase-3-plan.md](../plans/phase-3-plan.md) — closed by this ADR.
- [phase-3-stdlib-next-steps.md](../plans/phase-3-stdlib-next-steps.md),
  [phase-3-stdlib-round-2.md](../plans/phase-3-stdlib-round-2.md) —
  experiment plans whose outcomes this ADR records.
- [plans/phase-3-results/FREEZE_ADR_EXCERPT.md](../plans/phase-3-results/FREEZE_ADR_EXCERPT.md) —
  the option-3 framing this ADR adopts verbatim.
- [tacit-plan.md § Phase 4](../plans/tacit-plan.md) — the next phase.
- [tacit-plan.md § Phase 5](../plans/tacit-plan.md) — conditional
  follow-on; not committed.
