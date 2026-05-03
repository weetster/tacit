# Phase 3 Standard-Library Next Steps

**Status:** First canary complete; revise before any full open run
**Date:** 2026-05-02
**Updated:** 2026-05-03

## Summary

Run a library-mediated Tacit authoring experiment focused on the failure
clusters exposed by the open repair-loop run. This is a separate hypothesis
from primer-only core-language fluency:

> If Tacit supplies compact, general-purpose library operations for common
> buffer, sequence, text, and table work, can models write shorter and more
> reliable Tacit programs?

The answer must be reported separately from the Phase 3 one-shot primary gate
and the repair-loop result.

## Evidence

The full open repair-loop run `019de6ef-e75e-70d8-aa52-e98c4c577f7d`
improved from 30/47 one-shot passes to 40/47 final passes. The remaining
failures were concentrated in:

- sequence partitioning and formatting
- counting/grouping
- sorting
- matrix traversal
- line sorting

These are exactly the places where the current Tacit surface forces long
low-level buffer programs. The open reference corpus also shows the token
problem before primer cost: 20,661 Tacit tokens vs 4,584 Python tokens.

The first library-mediated canary,
`019debbe-3de9-7a54-8d59-cef6523672f1`, used Bundle A plus the narrow
line/token range subset of Bundle B. It improved same-task one-shot passes
from 1/12 to 4/12 against the latest non-stdlib repair-loop run's matching
task subset, and reduced generated tokens from 26,280 to 15,560. It did not
meet the full-open proceed criteria: final repair-loop passes were 8/12,
invalid recovery was 1/3, and average model calls were 2.17 per task. The
stdlib canary references were also only about 16% shorter than current Tacit
references on the 12-task subset, short of the 30% setup gate.

The failure pattern is actionable. The model used the new vector and range
primitives, but `@token-index text off len delim table` was too narrow for
ordinary tokenized input: models treated "token" as whitespace-delimited,
while the primitive splits on exactly one delimiter byte. That directly
caused newline-containing pseudo-tokens in grouping and empty-input failures
in sequence tasks. Sorting and grouping also remained too manual; indexed
storage helped token count, but did not make hand-written ordering logic
reliable enough.

## Non-Goals

- Do not reinterpret a stdlib-mediated pass as a primer-only Phase 3 pass.
- Do not tune from sealed tasks or inspect sealed contents.
- Do not add task-specific primitives such as `@solve-two-sum`.
- Do not change canonical syntax for this experiment.
- Do not overwrite the existing core-language Tacit references; keep a
  comparable baseline.

## Design Rules

Every candidate library addition must satisfy all of these:

- It is general-purpose across at least two task families or one obvious
  language-product domain.
- It has a compact authoring shape under `o200k_base`.
- It has a typed effect signature before implementation.
- It ships with codegen/typecheck tests and at least one model-facing example.
- It can be described without mentioning specific corpus task names.
- It reduces generated code size or failure rate in the canary before any
  full paid rerun.

## Surface Strategy

Tacit currently has no import/link story for user-level library modules.
Near-term stdlib expansion therefore has two layers:

1. **Primitive layer.** Add new `@name` operations when the operation cannot
   be expressed compactly in current Tacit or needs runtime support.
2. **Example/idiom layer.** Teach reusable patterns in the primer only after
   the primitive surface is stable and measured.

Longer term, a real module/import standard library should replace many
primitive additions. That is not required for this experiment.

## Candidate Bundles

### Bundle A — Buffer-Backed Vectors

Goal: stop encoding integer sequences through ad hoc byte loops.

Candidate operations:

- `@i64-get buf index`
- `@i64-set buf index value`
- `@i64-swap buf i j`
- `@i64-copy dst dst-index src src-index count`

Expected impact: shorter and safer sorting, matrix, partitioning, and
sequence-processing programs.

### Bundle B — Text Indexing

Goal: separate line/token discovery from per-task logic.

Candidate operations:

- `@line-index text len table`
- `@token-index text off len delim table`
- `@token-index-any text off len delims delim-count table`
- `@range-start table index`
- `@range-len table index`

The table can be represented as a buffer-backed vector of start/length pairs.
This depends on Bundle A or equivalent typed buffer access.

`@token-index-any` is the next required text primitive. It scans
`text[off..off+len)` into non-empty byte runs separated by any byte in
`delims[0..delim-count)`, writes absolute start/length pairs to `table`, and
returns the number of rows written. This avoids baking in a whitespace-only
special case while giving the model the operation it expected from
`@token-index`. The one-delimiter `@token-index` can remain as a compact
low-level primitive, but model-facing examples should prefer
`@token-index-any` for input records where spaces, LF, CR, or tabs may all
separate tokens.

Expected impact: shorter line sorting, unique-line, word-count, longest-line,
and substring-filtering programs.

### Bundle C — Ordering Primitives

Goal: avoid re-implementing fragile sorting loops in every program.

Candidate operations:

- `@sort-i64 buf count`
- `@sort-ranges-by-bytes text table count`
- `@stable-sort-pairs-i64 keys values count`

Expected impact: fewer compiler/codegen failures in sorting-heavy tasks and
large token reductions for generated programs.

### Bundle D — Search And Counting Helpers

Goal: provide reusable accumulation without pretending Tacit has closures or
hash maps yet.

Candidate operations:

- `@lower-bound-i64 buf count value`
- `@count-equal-ranges text table count out`
- `@dedup-adjacent-ranges text table count out`

Expected impact: shorter grouping, uniqueness, and lookup programs after
sorting or indexing.

## Canary

Use an open-only canary before any full paid stdlib run.

Primary target tasks:

- `collections/025-partition-eo`
- `collections/026-group-counts`
- `algorithms/035-bubble-sort`
- `algorithms/036-quicksort`
- `algorithms/037-merge-sort`
- `algorithms/049-matrix-multiply`
- `io/055-sort-lines`

Regression/coverage tasks:

- `collections/021-unique-in-order`
- `algorithms/033-two-sum`
- `algorithms/044-count-islands`
- `io/056-unique-lines`
- `strings/017-common-prefix`

This keeps the canary at 12 tasks and covers both final failures and tasks
that repair already recovered.

## Metrics

Report all of these for the canary and any later full run:

- one-shot pass rate
- final repair-loop pass rate
- invalid recovery rate
- behavioral recovery rate
- generation tokens by turn
- repair primer-inclusive token total
- stdlib-reference Tacit tokens vs current Tacit references
- stdlib-reference Tacit tokens vs Python
- generated-token reduction vs the latest non-stdlib repair-loop run

Do not use stdlib-mediated results to satisfy the existing Phase 3 primary
gate. Add a separate `library-mediated` label or result note.

## Exit Criteria

Proceed from design to implementation only if an ADR chooses a narrow first
bundle and defines exact signatures.

Proceed from implementation to paid canary only if:

- new primitive tests pass,
- existing open references still pass,
- stdlib canary references pass all tests,
- canary reference token count drops by at least 30% against current Tacit
  references, and
- the stdlib primer appendix is under 1,500 tokens.

Proceed from paid canary to full open run only if:

- one-shot canary pass count improves by at least 3 tasks,
- final repair-loop canary pass count is at least 11/12,
- generated tokens fall by at least 25% on the canary,
- invalid recovery is at least 50%, and
- average model calls stay below 2.0 per task.

The first paid canary did not satisfy this transition. Do not run the full
open library-mediated evaluation until a revised primitive set and primer
appendix clear these gates on a fresh open-only canary.

## Work Plan

1. Record the first canary as a library-mediated result note, separate from
   the primer-only Phase 3 gate and the core repair-loop result.
2. Write the next ADR amendment or follow-up ADR for Bundle B2:
   `@token-index-any text off len delims delim-count table`, including exact
   type/effect signatures and string-literal/buffer argument rules for
   `delims`.
3. Implement `@token-index-any` with codegen and typecheck fixtures. Include
   tests for leading, trailing, repeated, and mixed delimiters such as space
   plus LF.
4. Update the stdlib primer appendix to steer model-facing tokenization
   examples toward `@token-index-any` when input may contain more than one
   separator byte.
5. Rework the 12 canary `reference.stdlib.tac` files so the canary subset
   clears the 30% token-reduction setup gate before another paid run.
6. Add canary-subset token reporting, or document the exact `corpus-tokens`
   extraction method, because ADR 0021 `stdlib_dominated` buckets are not a
   useful aggregate for this library-mediated experiment.
7. Add the smallest ordering layer after B2, likely `@sort-i64` and
   `@sort-ranges-by-bytes`, if references still require long hand-written
   sorting/grouping loops.
8. Rerun the open canary one-shot and repair-loop modes only after local
   reference tests, token counts, and primer-size checks pass.
9. Decide whether to run full open stdlib-mediated evaluation based on the
   paid-canary gates above.

Step 2 implementation note: `corpus-eval` supports
`--result-label library-mediated`. The label is written into both run metadata
and metrics, and primary Phase 3 gates are reporting-only for labelled
library-mediated runs.

Step 5 implementation note: `corpus-tokens` reports `reference.stdlib.tac`
when present, with per-task `stdlib` / `stdΔ` / `std/tac` columns and aggregate
rows for stdlib Tacit references plus the paired stdlib-vs-current Tacit
subset.

## Post-Canary Recommendation

Do not proceed to a full open library-mediated run from the current Bundle A
plus narrow Bundle B surface. The first canary shows that indexed storage and
range tables reduce generated tokens, but the text-indexing shape is not yet
aligned with model behavior or ordinary corpus input.

Next, add `@token-index-any` and refresh the primer/examples around it. Then
recheck local reference token counts before paying for another canary. If the
updated references still require hand-written sorting or grouping loops,
promote the smallest ordering primitives from Bundle C before rerunning paid
evaluation.
