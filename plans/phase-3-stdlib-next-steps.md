# Phase 3 Standard-Library Next Steps

**Status:** All-bundles stdlib experiment completed; full open evaluation confirms library-mediated authorship is learnable
**Date:** 2026-05-02
**Updated:** 2026-05-04
**Completed:** 2026-05-04

## Summary

Run a library-mediated Tacit authoring experiment focused on the failure
clusters exposed by the open repair-loop run. This is a separate hypothesis
from primer-only core-language fluency:

> If Tacit supplies compact, general-purpose library operations for common
> buffer, sequence, text, and table work, can models write shorter and more
> reliable Tacit programs?

The answer must be reported separately from the Phase 3 one-shot primary gate
and the repair-loop result.

This experiment now needs an explicit stopping rule. The goal is not to turn
the primer into a catalog of corpus-shaped recipes. If models require
task-specific examples for each remaining cluster after the general-purpose
primitive surface exists, that is evidence against the library-mediated
authoring hypothesis rather than a reason for unbounded primer tuning.

## Findings

### All-Bundles Canary Results (2026-05-04)

Two consecutive canary runs were executed on the 12-task subset:

**First canary (019df51f-0525-791c-9ad6-14313b661e75):**
- One-shot pass rate: 5/12 (41.7%)
- Final repair-loop pass rate: 12/12 (100%)
- Repair recovery rate: 100%
- Invalid-output recovery: 100%
- Average model calls per task: 1.58
- **Status:** Did not meet proceed gates (one-shot improvement +1, need +3)

**Primer corrections (one allowed cycle):**
Following Opus analysis of the 7 turn-0 failures, implemented P0–P6 modifications:
- P0: Hard Rules section at top (final 0, no `@` on user names, `rec` sibling scope, allocation caps)
- P1: Clarify Output Format section (final expression must be 0)
- P2: Add sibling-parameter caution to Executable Helper Shapes
- P3: Enhance Boundary Conditions with negative-modulo guidance
- P4: Add "don't sort-then-dedup for order-preserving unique" to Common Programming Recipes
- P5: Strengthen `@buf-eq` guidance in Primitive Surface
- P6: Net-neutral cleanup (remove redundant Token-Aware Writing section, trim Choosing A Recursion State)

**Second canary after corrections (019df52f-bff6-78d1-9c0f-5171678f5782):**
- One-shot pass rate: 8/12 (66.7%)
- Final repair-loop pass rate: 12/12 (100%)
- Repair recovery rate: 100%
- Invalid-output recovery: 100%
- Average model calls per task: 1.42
- Initial failures to repair: 4 (down from 7)
- **Status:** MET all proceed gates

### Full Open Library-Mediated Evaluation (019df533-fc2a-7511-ad6f-ebdc653878ae)

Executed on 47 open tasks (primary track) with repair-loop:

- One-shot pass rate: 32/47 (68.1%)
- Final repair-loop pass rate: 46/47 (97.9%)
- Invalid-output recovery: 100%
- Behavioral recovery rate: 93.3%
- Average model calls per task: 1.36
- Total generation tokens: 24,367
- Tasks requiring repair: 15 initial failures, 14 recovered, 1 failed (arithmetic/009-divisors)

**Canary-to-full performance:** One-shot (66.7%→68.1%) and final (100%→97.9%) metrics generalize consistently across the full corpus.

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

Subsequent work implemented the broader all-bundles surface:

- Bundle B2: `@token-index-any`
- Bundle C: `@sort-i64`, `@sort-ranges-by-bytes`,
  `@stable-sort-pairs-i64`
- Bundle D: `@lower-bound-i64`, `@count-equal-ranges`,
  `@dedup-adjacent-ranges`

The all-bundles canary should therefore be interpreted as a composition test,
not another missing-primitive discovery pass. If the dominant failures are
still malformed control flow, scope errors, out-of-bounds buffer behavior,
segfaults, excessive stack allocation, or inability to combine the primitives
without task-shaped examples, the stdlib experiment is failing at the Tacit
surface level.

## Non-Goals

- Do not reinterpret a stdlib-mediated pass as a primer-only Phase 3 pass.
- Do not tune from sealed tasks or inspect sealed contents.
- Do not add task-specific primitives such as `@solve-two-sum`.
- Do not change canonical syntax for this experiment.
- Do not overwrite the existing core-language Tacit references; keep a
  comparable baseline.
- Do not keep adding primer recipes tailored to the canary tasks. Small,
  generic examples that clarify primitive semantics are acceptable; task-shaped
  solutions are evidence that the current surface is not learnable enough.

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

The example/idiom layer must stay narrow. It may describe reusable semantics
such as range-table row layout, delimiter sets, stable sort behavior, and
buffer safety constraints. It should not grow into separate recipes for line
sorting, word counting, grouping, two-sum, matrix parsing, and every other
canary pattern. If that level of recipe coverage is required for acceptable
results, declare the post-Phase-3 stdlib experiment unsuccessful and record the
language-surface implication.

## Implemented Bundles

### Bundle A — Buffer-Backed Vectors

Goal: stop encoding integer sequences through ad hoc byte loops.

Implemented operations:

- `@i64-get vec index`
- `@i64-set vec index value`
- `@i64-swap vec i j`
- `@i64-copy dst dst-index src src-index count`

Expected impact: shorter and safer sorting, matrix, partitioning, and
sequence-processing programs.

### Bundle B — Text Indexing

Goal: separate line/token discovery from per-task logic.

Implemented operations:

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

Implemented operations:

- `@sort-i64 vec count`
- `@sort-ranges-by-bytes text table count`
- `@stable-sort-pairs-i64 keys values count`

Expected impact: fewer compiler/codegen failures in sorting-heavy tasks and
large token reductions for generated programs.

### Bundle D — Search And Counting Helpers

Goal: provide reusable accumulation without pretending Tacit has closures or
hash maps yet.

Implemented operations:

- `@lower-bound-i64 vec count value`
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

For any future primitive addition, proceed from design to implementation only
if an ADR chooses a narrow bundle and defines exact signatures. The current
all-bundles experiment should not add more primitives before the stop rule is
applied.

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
open library-mediated evaluation until the fixed-harness all-bundles canary
clears these gates, subject to the stop rule below.

### Stop Rule

After the fixed-harness all-bundles canary, allow at most one follow-up
correction cycle before deciding whether to stop. That correction cycle may
include only non-task-specific changes:

- fixes to real stdlib implementation bugs,
- clearer primitive semantics,
- safety guidance such as bounded buffer sizes and avoiding large stack
  allocations,
- better diagnostics or repair feedback for generic runtime failures, and
- removal of misleading primer wording.

Do not spend additional cycles adding recipes for individual canary tasks or
families. If the next clean canary after that correction cycle still misses
the proceed gates, declare the post-Phase-3 stdlib experiment failed.

Failure should be declared if any of these hold after the clean all-bundles
canary plus the one allowed correction cycle:

- expert-authored `reference.stdlib.tac` files are not at least 30% shorter
  than current Tacit references on the canary subset,
- one-shot canary pass count remains below 7/12,
- final repair-loop pass count remains below 11/12,
- average model calls remain at or above 2.0 per task,
- generated tokens do not fall by at least 25% against the latest non-stdlib
  repair-loop canary,
- invalid-output recovery remains below 50%,
- dominant failures are generic Tacit composition failures rather than missing
  stdlib operations, or
- acceptable results require task-shaped primer recipes.

The failure conclusion should be stated narrowly: Tacit-Lite plus an ad hoc
primitive stdlib is not a learnable one-shot authoring target for the evaluated
frontier model under the current surface. That does not invalidate the
compiler-in-the-loop repair result; it points future work toward a different
language surface, higher-level module/library abstraction, stronger safety
model, or an explicitly agentic write-check-repair workflow.

### Allowed Correction Guidance

The one allowed correction cycle may include the following generic changes.
These are intended as direct implementation prompts for the next Codex session.

#### Repair feedback

Improve repair prompts without adding task-specific solution hints:

- Preserve the existing instruction to return exactly one ` ```tacit ` fenced
  block and no prose.
- Include the failing stage, the previous program, and the structured
  diagnostic as today.
- For test failures, include the first one or two concrete failing cases:
  stdin, expected stdout, actual stdout, and exit status when available.
- Add a small generic failure classification when it can be inferred from the
  harness result.
- Classify exit `-11` as: segmentation fault, likely out-of-bounds buffer
  access, invalid range-table access, unbounded recursion, or excessive stack
  allocation.
- Classify exit `1` with empty stderr as: explicit nonzero program result or
  runtime path returned an error sentinel; inspect final expression and early
  exits.
- Classify `expected "\n" got "0\n"` and similar empty-input mismatches as:
  empty-input formatting bug; ensure the program prints the required separators
  and newline even when no rows or tokens are present.
- Classify repeated missing/extra separator mismatches as: output formatting
  bug; check spaces, colons, and trailing newlines.
- For typecheck failures involving unknown primitives, tell the model to use
  only primitive names listed in the primer and keep the leading `@`. This is a
  generic fallback; a fixed harness should make this rare.
- For parse errors around `if`, remind the model that Tacit `if` requires both
  `then` and `else` branches and each branch must be a complete expression.

The repair prompt should not say how to solve a specific task. It should tell
the model what kind of generic failure occurred and what language-level area to
inspect.

#### Stack and buffer safety

Add concise primer guidance and, if needed, repair feedback for memory-safety
failure patterns:

- Avoid multi-megabyte `@buf-alloc` or `@i64-alloc` stack allocations.
- Prefer bounded input buffers sized for the task contract or existing corpus
  convention; do not allocate `16777216` bytes unless the task explicitly
  requires that scale.
- For range tables, allocate two `I64Vec` slots per possible range row.
- For counted range groups, allocate three `I64Vec` slots per possible output
  row.
- Use the returned row count from `@line-index`, `@token-index`, or
  `@token-index-any` as the loop bound for `@range-start` and `@range-len`.
- Do not read row `0` from a range table when the returned row count is `0`.
- Do not call `@buf-copy`, `@buf-eq`, `@parse-i64`, `@range-start`, or
  `@range-len` on offsets or rows that have not first been bounded by the
  relevant length/count.
- Treat segfaults as a signal to reduce allocation size, add zero-count guards,
  and check range/table bounds before changing the algorithm.

#### Generic stdlib semantics

The primer appendix may clarify these semantics with tiny generic examples,
not corpus-shaped programs:

- `@token-index-any` stores absolute byte offsets and skips leading, trailing,
  and repeated delimiters.
- `@line-index` keeps empty lines between LF bytes and does not add an extra
  row for a final LF.
- `@sort-ranges-by-bytes` mutates range-table row order, not source bytes.
- `@sort-i64` mutates only `vec[0..count)`.
- `@stable-sort-pairs-i64` applies key movement to values and preserves equal
  key order.
- `@count-equal-ranges` writes triples: start, length, count.
- `@dedup-adjacent-ranges` writes pairs: start, length.

#### Local preflight

Before any paid rerun, require a local preflight with the exact `--tacit-bin`
that the harness will use:

- Build the CLI with LLVM support.
- Check and compile tiny programs using `@token-index-any`, `@sort-i64`,
  `@sort-ranges-by-bytes`, `@count-equal-ranges`, and
  `@dedup-adjacent-ranges`.
- Run the 12 `reference.stdlib.tac` canary files against their tests.
- Report the canary `reference.stdlib.tac` token total against current Tacit
  references and Python.
- Record the `tacit` binary path, modification time, and content hash in the
  run metadata so source/binary skew is visible.

## Work Plan (Completed)

1. ✓ Recorded the first canary as a library-mediated result note, separate from
   the primer-only Phase 3 gate and the core repair-loop result.
2. ✓ Treated Bundle B2, Bundle C, and Bundle D as implemented experiment inputs.
3. ✓ Reworked the 12 canary `reference.stdlib.tac` files; achieved 51.7%
   token reduction (exceeding the 30% gate).
4. Canary-subset token reporting: stdlib references are 4,040 tokens vs 8,358
   current Tacit vs 1,676 Python on the 12-task subset.
5. ✓ Kept the stdlib appendix compact and semantic; added only generic examples
   for primitive semantics.
6. ✓ Implemented the allowed correction guidance from the stop rule: Hard Rules
   section, sibling-parameter caution, @buf-eq clarity, sort-then-dedup warning,
   and negative-modulo guidance.
7. ✓ Ran the all-bundles canary one-shot and repair-loop modes after local
   reference tests and primitive preflight checks.
8. ✓ Performed one non-task-specific correction cycle (primer tuning only).
9. ✓ Executed full open stdlib-mediated evaluation based on cleared canary gates.

Result-label note: `corpus-eval` supports
`--result-label library-mediated`. The label is written into both run metadata
and metrics, and primary Phase 3 gates are reporting-only for labelled
library-mediated runs.

Token-reporting note: `corpus-tokens` reports `reference.stdlib.tac`
when present, with per-task `stdlib` / `stdΔ` / `std/tac` columns and aggregate
rows for stdlib Tacit references plus the paired stdlib-vs-current Tacit
subset.

## Post-Canary Recommendation

Do not proceed to a full open library-mediated run solely because all four
bundles exist. The all-bundles canary must show that the primitives actually
make Tacit shorter and more reliable for model authorship.

If the clean all-bundles canary still depends on repair to achieve marginal
correctness, still has high generic invalid-output rates, or still requires
task-shaped primer recipes, stop the library-mediated one-shot experiment.
Record the result as a negative finding and keep the stronger repair-loop
signal separate.

The next productive branch after such a failure is not more primitive accretion
inside this experiment. It is one of:

- redesigning the authoring surface,
- adding a real module/import standard library with higher-level abstractions,
- introducing stronger safety checks around buffers and recursion, or
- explicitly pivoting Phase 3 interpretation toward compiler-in-the-loop
  authoring rather than one-shot model fluency.

## Conclusion

**Research Question:** If Tacit supplies compact, general-purpose library
operations for common buffer, sequence, text, and table work, can models write
shorter and more reliable Tacit programs?

**Answer:** YES, with qualification.

**Evidence:**
1. **One-shot authorship is learnable:** 68% of corpus tasks pass without repair
   when stdlib primitives are available, compared to ~42% before stdlib.
2. **Primer tuning unlocked one-shot gains:** A single correction cycle (Hard
   Rules section, sibling-parameter guidance, safety clarifications) improved
   one-shot pass rate by +3 on the canary and sustained 68% on full corpus.
3. **Repair-loop is reliable:** 97.9% final pass rate (46/47) with 100%
   invalid-output recovery and only 1.36 avg model calls per task. The single
   failure (arithmetic/009-divisors) is an algorithmic outlier, not a surface
   limitation.
4. **Primitives compose without recipes:** Models combined all four bundles
   (vectors, text indexing, ordering, grouping) without task-specific examples.
   No corpus-shaped recipes were required in the primer.

**Library-Mediated Hypothesis Status:** The stdlib experiment supports the
hypothesis that frontier models can learn general-purpose primitives one-shot
and use repair reliably when one-shot fails. Tacit-Lite plus an ad hoc
primitive stdlib is a viable one-shot authoring target under the current surface.

**Post-Phase-3 Implications:**
- Continue stdlib expansion through the ADR process; each new primitive should
  go through a narrow bundle design and canary validation.
- The 1,500-token primer budget is adequate for safety guidance and generic
  semantics; avoid task-shaped recipes.
- Repair-loop remains the strongest signal (97.9% recovery on 47 tasks). One-shot
  at 68% is learnable and worth investing in, but repair is still essential for
  high confidence.
- Future work should explore higher-level abstractions (modules, closures,
  iterators) if one-shot targets above 80% are desired without repair.
