# Phase 5 Open Maintenance Benchmark

This directory is the Stage 1 deliverable for Phase 5. It defines a small,
open maintenance/debug benchmark that exercises the Phase 4-era workflow
without reading, listing, searching, or otherwise accessing
`corpus/sealed/`.

Each task has:

- an authoritative canonical source file: `main.tac`
- a benchmark-facing authoring render: `main.taca`

The benchmark is intentionally narrow:

- 4 repair tasks
- 2 edit tasks
- 2 explanation tasks

That keeps the gate small enough to run quickly while still covering compile,
type, behavior, and explanation work.

## Scope and Safety

- Only files under `plans/phase-5-benchmark/` are benchmark inputs.
- Source examples in `examples/phase-4/` and `examples/smoke/` were used as
  design references while authoring these open tasks, but they are not
  required at run time.
- The presence of checked-in `.taca` files here is a benchmark exception for
  Phase 5 evaluation input. It does not change the repository-wide storage
  policy that treats canonical `.tac` as authoritative and `.taca` as a
  transient or historical form.
- Benchmark runs must not read, list, search, or reveal anything under
  `corpus/sealed/`.
- Operators should not use repository-wide globs rooted at `corpus/` for this
  benchmark. The benchmark is fully self-contained here.

## Common Allowed Tool Surface

Every task uses the same allowed Phase 4-era tool surface:

- `tacit check <file>`
- `tacit compile <file> -o <output>`
- `tacit view <file> --as inspection --types --effects`
- Running the compiled program to observe exit status and stdout
- Repository search and file reads outside `corpus/sealed/`

No task assumes a new debugger, diff tool, blame tool, IDE integration, or
any Phase 6 or Phase 7 surface.

## Scored Surface

Stage 2 should score the benchmark primarily on the authoring-facing input
surface, using the checked-in `main.taca` files as the task handoff artifact.
That is the more decision-useful measure of maintenance workflow quality.

The paired `main.tac` files remain the authoritative repository form and may be
used as a secondary control run or substrate check, but canonical-only results
must be reported separately from the primary authoring-surface score.

## Run Procedure

From the repository root:

1. Work only inside `plans/phase-5-benchmark/`.
2. For repair and edit tasks, use `tacit check`, `tacit compile`, inspection
   view, and program execution until the pass condition is met.
3. For explanation tasks, do not edit the file. Produce a written explanation
   grounded in the observed diagnostics and, when useful, inspection output.
4. For the primary scored run, start from `main.taca`. If a canonical control
   run is also performed, record it as a separate surface in the Stage 2
   artifact.
5. Record prompts, diagnostics, outputs, and final results under the eventual
   Stage 2 run artifact directory.

## Task Summary

| ID | Class | Canonical source | Authoring input | Pass condition | Grading focus |
| --- | --- | --- | --- | --- | --- |
| `r1-record-total` | Repair | `r1-record-total/main.tac` | `r1-record-total/main.taca` | Compile succeeds; exit `9`; stdout empty | Behavioral recovery |
| `r2-closure-offset` | Repair | `r2-closure-offset/main.tac` | `r2-closure-offset/main.taca` | Compile succeeds; exit `42`; stdout empty | Behavioral recovery |
| `r3-record-field` | Repair | `r3-record-field/main.tac` | `r3-record-field/main.taca` | Compile succeeds; exit `33`; stdout empty | Compile and type recovery |
| `r4-map-destination` | Repair | `r4-map-destination/main.tac` | `r4-map-destination/main.taca` | Compile succeeds; exit `18`; stdout empty | Behavioral recovery |
| `e1-record-bonus` | Edit | `e1-record-bonus/main.tac` | `e1-record-bonus/main.taca` | Compile succeeds; exit `35`; stdout empty | Small feature edit |
| `e2-closure-scale` | Edit | `e2-closure-scale/main.tac` | `e2-closure-scale/main.taca` | Compile succeeds; exit `42`; stdout empty | Small feature edit |
| `x1-missing-record-field` | Explanation | `x1-missing-record-field/main.tac` | `x1-missing-record-field/main.taca` | Correct explanation of why the program fails | Explanation correctness |
| `x2-non-function-map` | Explanation | `x2-non-function-map/main.tac` | `x2-non-function-map/main.taca` | Correct explanation of why the program fails | Explanation correctness |

## Task Specs

### `r1-record-total`

- Class: Repair
- Starting files:
  `plans/phase-5-benchmark/r1-record-total/main.taca` for the primary scored
  run, with `plans/phase-5-benchmark/r1-record-total/main.tac` retained as the
  authoritative canonical pair.
- Prompt:
  Fix the existing program so it returns the intended accumulator result. The
  program should compile cleanly and exit with status `9`. Keep the current
  record-based structure; do not rewrite it into a different algorithm.
- Allowed tools: common allowed tool surface
- Pass condition:
  `tacit compile` succeeds and the resulting program exits `9` with no stdout.
- Grading expectations:
  Behavioral recovery only. The agent should identify a wrong final projection
  or wrong final arithmetic combination rather than changing the overall
  accumulator structure.

### `r2-closure-offset`

- Class: Repair
- Starting files:
  `plans/phase-5-benchmark/r2-closure-offset/main.taca` for the primary scored
  run, with `plans/phase-5-benchmark/r2-closure-offset/main.tac` retained as
  the authoritative canonical pair.
- Prompt:
  Repair the closure pipeline so the returned closure adds the captured offset
  to its input. The fixed program must compile and exit with status `42`.
- Allowed tools: common allowed tool surface
- Pass condition:
  `tacit compile` succeeds and the resulting program exits `42` with no
  stdout.
- Grading expectations:
  Behavioral recovery. The fix should preserve the current returned-closure
  shape and correct the variable usage inside the nested lambda.

### `r3-record-field`

- Class: Repair
- Starting files:
  `plans/phase-5-benchmark/r3-record-field/main.taca` for the primary scored
  run, with `plans/phase-5-benchmark/r3-record-field/main.tac` retained as the
  authoritative canonical pair.
- Prompt:
  The program is meant to project a value from a small record and return it.
  Repair the failing program with the smallest coherent change so it compiles
  and exits `33`.
- Allowed tools: common allowed tool surface
- Pass condition:
  `tacit compile` succeeds and the resulting program exits `33` with no
  stdout.
- Grading expectations:
  Compile and type recovery. The expected fix is a record-field correction, not
  a rewrite of the program shape.

### `r4-map-destination`

- Class: Repair
- Starting files:
  `plans/phase-5-benchmark/r4-map-destination/main.taca` for the primary
  scored run, with `plans/phase-5-benchmark/r4-map-destination/main.tac`
  retained as the authoritative canonical pair.
- Prompt:
  This vector `map` program compiles but returns the wrong result. Repair it so
  it uses the mapped output when computing the final sum. The fixed program
  must exit `18`.
- Allowed tools: common allowed tool surface
- Pass condition:
  `tacit compile` succeeds and the resulting program exits `18` with no
  stdout.
- Grading expectations:
  Behavioral recovery. The repair should preserve the `map` workflow and fix
  the final read path.

### `e1-record-bonus`

- Class: Edit
- Starting files:
  `plans/phase-5-benchmark/e1-record-bonus/main.taca` for the primary scored
  run, with `plans/phase-5-benchmark/e1-record-bonus/main.tac` retained as the
  authoritative canonical pair.
- Prompt:
  Extend the working program so it returns `value + bonus` from the existing
  record. The edited program must compile and exit `35`.
- Allowed tools: common allowed tool surface
- Pass condition:
  `tacit compile` succeeds and the resulting program exits `35` with no
  stdout.
- Grading expectations:
  Small feature edit over an already working record program. The change should
  reuse the existing record and projection style.

### `e2-closure-scale`

- Class: Edit
- Starting files:
  `plans/phase-5-benchmark/e2-closure-scale/main.taca` for the primary scored
  run, with `plans/phase-5-benchmark/e2-closure-scale/main.tac` retained as
  the authoritative canonical pair.
- Prompt:
  Extend the working closure pipeline so the returned closure doubles its input
  before adding the captured offset. Keep the returned-closure structure. The
  final program must compile and exit `42`.
- Allowed tools: common allowed tool surface
- Pass condition:
  `tacit compile` succeeds and the resulting program exits `42` with no
  stdout.
- Grading expectations:
  Small feature edit over a working closure example. The expected change is in
  the nested lambda body and call setup, not a rewrite into a first-order
  program.

### `x1-missing-record-field`

- Class: Explanation
- Starting files:
  `plans/phase-5-benchmark/x1-missing-record-field/main.taca` for the primary
  scored run, with `plans/phase-5-benchmark/x1-missing-record-field/main.tac`
  retained as the authoritative canonical pair.
- Prompt:
  Do not edit the file. Explain why the program fails, using the structured
  diagnostic output and, if helpful, the inspection view. The explanation must
  identify the failing projection and why the record does not support it.
- Allowed tools: common allowed tool surface
- Pass condition:
  The explanation correctly states that the program projects a field that is
  not present on the record and points to the failing expression.
- Grading expectations:
  Explanation correctness only. The benchmark records whether the explanation
  is technically correct and specific enough for a human reviewer to act on.

### `x2-non-function-map`

- Class: Explanation
- Starting files:
  `plans/phase-5-benchmark/x2-non-function-map/main.taca` for the primary
  scored run, with `plans/phase-5-benchmark/x2-non-function-map/main.tac`
  retained as the authoritative canonical pair.
- Prompt:
  Do not edit the file. Explain why the `map` call fails, using the structured
  diagnostic output and, if helpful, the inspection view. The explanation must
  identify the invalid callback position and the expected callback shape.
- Allowed tools: common allowed tool surface
- Pass condition:
  The explanation correctly states that `map` expects a function callback and
  that the supplied callback position contains a non-function value.
- Grading expectations:
  Explanation correctness only. The explanation should identify the combinator
  contract rather than only restating that compilation failed.
