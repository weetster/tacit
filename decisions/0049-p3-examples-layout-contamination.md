# 0049 — `examples/phase-3/` layout and primer-contamination boundary

**Status:** Accepted
**Date:** 2026-04-28
**Phase:** 3, Stage 1
**Closes:** [phase-3-plan.md Q-P3-3](../plans/phase-3-plan.md)

## Context

[phase-3-plan.md § Stage 3](../plans/phase-3-plan.md) carries over three
non-trivial programs from Phase 2 — one sorting algorithm, one
linked-list-style data structure, one file-I/O program beyond `echo` —
per [ADR 0046 § 3](0046-p2-stage-5-frozen.md). Those programs live in
the repo as Tacit-Lite source plus sidecar plus a CI-runnable test, the
same shape as
[`examples/smoke/`](../examples/smoke/).

[phase-3-plan.md § Stage 4–6](../plans/phase-3-plan.md) also lands
hand-authored Tacit-Lite references for each of the 47 open corpus
tasks, in the layout fixed by [ADR 0048](0048-p3-tacit-idiom-rules.md)
(`corpus/tasks/<category>/<NNN-slug>/reference.tac`).

[phase-3-plan.md § Stage 7](../plans/phase-3-plan.md) authors the
primer, which draws progressive Python ↔ Tacit-Lite examples from the
repo. The primer is the load-bearing artifact for the Phase 3 model
under test: every snippet the model sees comes from somewhere in
the repo.

This produces a contamination question. The primer is the input to a
falsification test on the open corpus. If the primer contains any
verbatim corpus reference, the test devolves into "can the model copy
text from its context window," which is not the thesis claim. If the
primer touches sealed material at all, the held-out subset under
[ADR 0020](0020-sealing-held-out-in-repo.md) is leaked. The boundary
between primer source material and corpus reference material must be
written down.

A second ambiguity: the Stage 3 carry-over programs and the Stage 4–6
corpus references are *both* hand-authored Tacit-Lite, *both* live
in the repo, and *both* are fair game for primer prose. Without a
naming and discipline rule, an author writing the primer might pull
a sorting example from `corpus/tasks/algorithms/036-quicksort/reference.tac`
when they should pull it from `examples/phase-3/`. The result is the
same thesis-degeneracy as before — the model has seen the corpus
reference verbatim.

## Decision

**Phase 2 carry-over programs live in `examples/phase-3/`, parallel to
`examples/smoke/`. The primer draws code examples exclusively from
`examples/smoke/` and `examples/phase-3/`; it never draws verbatim
text from `corpus/tasks/<...>/reference.tac` and never references
sealed material at all.**

### Directory layout

```
examples/
  smoke/                  Phase 1 smoke corpus (frozen by ADR 0033).
  phase-3/                Phase 2 carry-over programs (this ADR).
    README.md             Lists the three programs with effect signatures.
    sort.tac              Working name; concrete picks at Stage 3.
    sort.tac.sidecar.toml
    list.tac
    list.tac.sidecar.toml
    sum-numbers.tac
    sum-numbers.tac.sidecar.toml
corpus/
  tasks/                  Phase 0 corpus (frozen by ADR 0020 + ADR 0019).
    <category>/<NNN-slug>/
      task.md             Task statement (open or sealed).
      tests.jsonl         Test cases (open or sealed).
      reference.py        Python reference (per ADR 0019).
      reference.rs        Rust reference (per ADR 0019).
      reference.tac       Tacit-Lite reference (per ADR 0048; open only).
      reference.tac.sidecar.toml
  sealed/                 Sealed bodies for held-out tasks (ADR 0020).
```

`examples/phase-3/` is a sibling of `examples/smoke/`, not nested
inside it. Both are open content, hand-authored, and CI-tested.

### Naming discipline

- The three Stage 3 programs use **distinct names from any open
  corpus task slug**. `sort.tac` is acceptable; `bubble-sort.tac`
  is not (open corpus task 035-bubble-sort exists). `list.tac` is
  acceptable; `linked-list.tac` is fine; nothing else collides.
  `sum-numbers.tac` collides with `corpus/tasks/io/052-sum-numbers/`
  by *name*; the program is **not** the same program (the
  `examples/phase-3/` version uses different input format and
  different output format) and the README documents the
  divergence. Working assumption is that this collision is
  acceptable; if Stage 3 finds it confusing, the file is renamed.
- The Stage 3 programs do **not** duplicate the algorithmic shape
  of the open corpus task they share a category with. The
  sort program implements an algorithm distinct from any of
  open tasks 035 (bubble-sort), 036 (quicksort), 037 (merge-sort);
  insertion or selection sort is the working assumption per
  [phase-3-plan.md § Stage 3](../plans/phase-3-plan.md). The
  same rule applies to list and sum-numbers.

### Primer source rule

The primer (under [ADR 0050](0050-p3-primer-scope.md)) draws code
examples from exactly three sources:

1. **`examples/smoke/`** — every program. Frozen content, fair game.
2. **`examples/phase-3/`** — every program. Frozen at Stage 3 exit.
3. **Original prose snippets authored for the primer** — small
   fragments illustrating syntax or one-shot primitives.

The primer **does not** draw from:

- **Any `corpus/tasks/<...>/reference.tac`** — open or sealed. The
  open Tacit references are eval material, not primer material.
  This is the central anti-contamination rule. A model that has
  seen `036-quicksort/reference.tac` verbatim in its context will
  pass that task by copying. The primer must not provide that
  copy.
- **Any `corpus/sealed/`** — under any access path. Sealed integrity
  is enforced by `corpus-verify-sealed` and by the
  `.claude/settings.json` denials; this rule is the authorial
  posture that backs them.
- **Any `corpus/tasks/<...>/task.md`** as a worked example.
  Referencing a task statement to motivate a section is fine
  ("the open corpus has a binary-search task; here is how a
  binary-search-shaped algorithm is written in Tacit-Lite, using
  a different shape than the corpus reference"), but lifting the
  task's prose is not. Mirror of ADR 0019's "no docstring leaks"
  rule.

### Why open Tacit references are off-limits to the primer

The open subset (47 tasks) is not sealed; it is public; and ADR
0019 § Authorship explicitly allows the primer to draw from
open *Python* references. The Tacit references are different in
one specific way: they are the answer key for the Tacit-Lite
half of the eval. A model that copies a Tacit reference verbatim
passes the task without demonstrating language fluency. The
Python references do not have this property because the model is
not asked to *generate Python*; it is asked to generate
Tacit-Lite, with the Python reference as a shape hint.

So the rule is: **primer may show how Python ↔ Tacit translates,
using prose that resembles a corpus reference at the algorithmic
level. It may not paste the corpus reference itself.** A primer
section "here is how list reversal looks in Tacit-Lite" may show
a list-reversal snippet drawn from `examples/phase-3/list.tac` or
written fresh; it may not be `corpus/tasks/strings/011-reverse-string/reference.tac`
verbatim or with cosmetic edits.

### Mechanical enforcement

The Stage 7 primer-fixture test
([phase-3-plan.md § Stage 7 exit gate](../plans/phase-3-plan.md))
extracts every fenced Tacit-Lite block from the primer and:

1. Compiles each block via the repo-local `tacit` binary
   (catching syntax / type / effect regressions).
2. Hashes each block (BLAKE3) and compares against the hashes of
   every `corpus/tasks/<...>/reference.tac` and every file under
   `corpus/sealed/`. Any match is a hard CI failure.
3. Tokenises each block under tiktoken `o200k_base` and asserts
   no contiguous 32-token run matches any corpus reference. The
   32-token threshold catches near-verbatim copies that escape
   exact hash equality (e.g., one renamed local).

The 32-token window is calibrated against the smallest corpus
reference (estimated 40–60 tokens for trivial tasks); 32 is short
enough to catch a copy of the body of any non-trivial reference
and long enough to admit shared idioms (a 3-token `@parse-i64`
call site, a 5-token `lambda x. @add x 1` skeleton).

### Effect-signature requirement for `examples/phase-3/`

Per [ADR 0046 § 3](0046-p2-stage-5-frozen.md), each Stage 3 program
typechecks with its effect signature verified. The README at
`examples/phase-3/README.md` lists all three programs with their
type and effect annotations, mirroring the `examples/smoke/README.md`
table.

### CI integration

`examples/phase-3/` programs are exercised by the existing
end-to-end smoke step in `.github/workflows/ci.yml`, extending the
`examples/smoke/` invocation to include Phase 3 programs. The CI
step compiles, links, runs, and asserts the expected stdout / exit
code per program.

## Alternatives considered

- **Put Phase 2 carry-over programs in `examples/smoke/`.** Rejected.
  The smoke corpus is frozen by [ADR 0033](0033-phase-1-frozen.md);
  adding to it is a spec amendment with no upside. A separate
  directory keeps Phase 1 and Phase 3 contributions auditable in
  isolation.
- **Put corpus references in `examples/phase-3/`.** Rejected. The
  corpus references are eval-half material; bundling them with
  primer-source material smears the contamination boundary. The
  per-task layout is also load-bearing for the harness's
  `corpus-run-tacit` command (per
  [phase-3-plan.md § Stage 4](../plans/phase-3-plan.md)).
- **Allow the primer to draw from open Tacit references with cosmetic
  edits** (renamed locals, reformatted whitespace). Rejected. The
  32-token window in the mechanical check is set deliberately tight;
  cosmetic edits are exactly the failure mode the rule is preventing.
  If a primer section needs an example that an open reference also
  uses, the primer authors a fresh example or pulls from
  `examples/phase-3/` / `examples/smoke/`.
- **Lift the rule for sealed content too** (forbid only verbatim
  copies, not all references). Rejected immediately. Sealed material
  is sealed by ADR 0020; the `.claude/settings.json` denial and
  `corpus-verify-sealed` enforce it; this ADR's posture mirrors
  those.
- **Make the carry-over programs net-new corpus tasks** instead of
  smoke-style examples. Rejected. The corpus is frozen at 60 tasks
  by [ADR 0020](0020-sealing-held-out-in-repo.md). Adding tasks
  reopens corpus design; the carry-over scope is "demonstrate
  non-trivial programs typecheck and run," which is the smoke-style
  pattern.
- **Skip the 32-token contiguous-run check** and rely only on
  exact-hash equality. Rejected. The hash check catches verbatim
  copies; the 32-token check catches near-verbatim. Skipping the
  weaker check is a known false-negative path on a load-bearing
  rule.

## Consequences

- **The carry-over programs have a clear home.** Stage 3 lands
  three programs under `examples/phase-3/` with the smoke-corpus
  shape; Phase 2's exit criterion 2 is satisfied without
  contaminating the corpus tree.
- **Primer authoring has a hard rule.** Stage 7 authors know which
  files they can lift from. Edge cases (a primer needs to show
  list reversal; the corpus also has a list-reversal task) are
  resolved by writing a fresh snippet or pulling from
  `examples/phase-3/`.
- **The contamination check is mechanical.** Stage 7's primer
  fixture is a CI gate, not a review judgment. Authors get
  immediate feedback on a bad snippet rather than a Stage 9
  baseline-run surprise.
- **Open Tacit references are eval-only.** Authors who write the
  Stage 4–6 references know those references exist solely to drive
  `corpus-run-tacit` and to feed `corpus-tokens`; they do not feed
  primer prose. This is a separation of concerns the harness
  relies on.
- **Sealed integrity is preserved.** This ADR's rule is consistent
  with ADR 0020's posture; `corpus-verify-sealed` continues to be
  the load-bearing CI gate for sealed file integrity.

## Related decisions

- [ADR 0019](0019-corpus-idiom-rules.md) — Python reference
  authorship rule that this ADR mirrors (with the verbatim-copy
  exception specific to the Tacit half of the eval).
- [ADR 0020](0020-sealing-held-out-in-repo.md) — sealing discipline;
  this ADR's "primer never touches sealed" rule is the authorial
  backing of the mechanical check.
- [ADR 0033](0033-phase-1-frozen.md) — `examples/smoke/` is frozen;
  Phase 3 carry-over goes to a sibling directory.
- [ADR 0046 § 3](0046-p2-stage-5-frozen.md) — the carry-over
  contract this ADR implements directory-wise.
- [ADR 0048](0048-p3-tacit-idiom-rules.md) — corpus reference
  layout; this ADR documents the relationship between
  `examples/phase-3/` and that layout.
- [ADR 0050](0050-p3-primer-scope.md) — primer scope; the source
  rule above is enforced at the primer-fixture level.
- [phase-3-plan.md § Stage 3, Stage 7](../plans/phase-3-plan.md)
  — implementation surfaces this ADR scopes.
