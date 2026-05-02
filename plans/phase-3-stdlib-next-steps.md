# Phase 3 Standard-Library Next Steps

**Status:** Proposed next experiment
**Date:** 2026-05-02

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
- `@range-start table index`
- `@range-len table index`

The table can be represented as a buffer-backed vector of start/length pairs.
This depends on Bundle A or equivalent typed buffer access.

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

## Work Plan

1. Write ADR 0061 choosing the first stdlib bundle and exact primitive
   signatures.
2. Add harness support for a `library-mediated` result label, or document the
   label convention if no code change is needed.
3. Implement the first bundle with codegen and typecheck fixtures.
4. Add alternate open canary references, for example
   `reference.stdlib.tac`, without replacing `reference.tac`.
5. Extend token tooling to compare `reference.tac`,
   `reference.stdlib.tac`, and `reference.py`.
6. Add a short stdlib primer appendix and fixture checks for every new
   example.
7. Run the open canary one-shot and repair-loop modes.
8. Decide whether to run full open stdlib-mediated evaluation.

## First Recommendation

Start with Bundle A plus the smallest part of Bundle B needed to represent
line/token ranges. This attacks the largest source of low-level repetition
without immediately adding high-level task-like operations.

Do not start with sorting primitives. Sorting should be the second bundle:
first make sequences and range tables compact and safe, then add ordering on
top of those representations.
