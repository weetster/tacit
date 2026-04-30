# Phase 3 Evaluation Corpus

This directory holds the Tacit Phase 3 evaluation corpus. It is a Phase 0
Stage 4 deliverable per [plans/phase-0-plan.md § Stage 4](../plans/phase-0-plan.md):
a frozen set of 50–100 tasks with reference solutions in Python and Rust and
executable test cases, against which Tacit-Lite is graded in Phase 3.

The idiom rules for reference solutions are pinned in
[ADR 0019](../decisions/0019-corpus-idiom-rules.md). The Python reference is
the sole token-count baseline for Phase 3's "at least 30% fewer tokens than
equivalent Python" exit criterion. [ADR 0021](../decisions/0021-corpus-stdlib-dominance-reporting.md)
adds a per-task `stdlib_dominated` tag (stored in
[stdlib-dominance.toml](stdlib-dominance.toml)) so `corpus-tokens` can
report the full, stdlib-dominated, and non-stdlib-dominated aggregates
separately — the 30% target is evaluated against the full *and*
non-stdlib-dominated aggregates.

## Status

**In progress (Phase 0 Stage 4).** Target at freeze: ~60 tasks total, with
~20% sealed as held-out. Current task count: see [MANIFEST.md](MANIFEST.md).

No Tacit-Lite references exist yet — this is the *target*, not a present
artifact. Tacit solutions are a Phase 3 deliverable.

## Layout

```
corpus/
├── README.md            — this file
├── MANIFEST.md          — task index by category, with open/held-out marks
├── held-out.txt         — IDs of sealed held-out tasks (advisory index)
├── sealed-hashes.txt    — BLAKE3 per file under sealed/; load-bearing tamper check
├── tasks/               — OPEN tasks (default scope for every harness command)
│   ├── arithmetic/      — IDs 001–010
│   ├── strings/         — IDs 011–020
│   ├── collections/     — IDs 021–030
│   ├── algorithms/      — IDs 031–050
│   └── io/              — IDs 051–060
├── sealed/              — HELD-OUT tasks; excluded by default, opt-in at grading time
│   ├── README.md        — sealing rules, do-not-read warning
│   └── <category>/<NNN-slug>/  — mirrors the tasks/ layout
└── harness/             — uv-managed Python project (test runner, token counter, sealed-verify)
```

Each task lives in its own directory under the appropriate category, named
`NNN-slug/` (three-digit zero-padded ID, kebab-case slug). Each task directory
contains exactly:

- `task.md`        — problem statement, stdin/stdout contract, constraints
- `tests.jsonl`    — one JSON object per line, fields `name`/`stdin`/`stdout`
- `reference.py`   — Python 3.12 reference solution
- `reference.rs`   — Rust 2024 reference solution

Task IDs are globally unique across categories and never renumbered once
assigned, even if a task is retired. Retired IDs remain in `MANIFEST.md`
marked `retired` so the numbering stays stable across freezes.

## Task contract

Every task is a stdin/stdout program. The program reads its input from
standard input and writes its output to standard output. No command-line
arguments, no environment variables, no filesystem access.

Each `tests.jsonl` file contains one test case per line:

```json
{"name": "basic", "stdin": "5\n", "stdout": "15\n"}
```

Fields:

- `name`   — short identifier, unique within the task
- `stdin`  — exact bytes fed to the program (use `\n` for newlines)
- `stdout` — exact bytes the program must write (compared byte-for-byte)

Exit code is expected to be zero unless a task explicitly specifies
otherwise. Stderr is ignored.

## Reference-solution rules (summary)

Full rules in [ADR 0019](../decisions/0019-corpus-idiom-rules.md). Quick
reference:

- **Python**: 3.12, stdlib only, `ruff format` and `ruff check` clean,
  type hints on function signatures, no docstrings, no defensive asserts,
  comprehensions where natural.
- **Rust**: 2024 edition, stdlib only, `cargo fmt` and `cargo clippy -D
  warnings` clean (default lint level), no `unsafe`, `?` on fallible paths
  in the solution proper, `unwrap()` only in the `main` harness.

Both references must produce byte-identical output on every test case.

## Held-out subset

Roughly 20% of tasks are sealed as held-out. These must not appear in
primer examples, training material, or any Phase 3 context visible to the
model under evaluation.

Sealing mechanism — per [ADR 0020](../decisions/0020-sealing-held-out-in-repo.md):

1. **Directory separation.** Held-out tasks live under `sealed/`, not
   `tasks/`. The two trees mirror each other's category layout.
2. **Hash manifest.** `sealed-hashes.txt` records a BLAKE3 per file.
   `uv run corpus-verify-sealed` fails if any file under `sealed/` is
   missing, extra, or modified. CI runs this on every push.
3. **Default-exclude harness.** `corpus-run` and `corpus-tokens` skip
   `sealed/` unless `--include-sealed` is passed. Any future tool
   walking the corpus follows the same convention.
4. **Claude Code denies.** [.claude/settings.json](../.claude/settings.json)
   denies Read/Edit/Write on `corpus/sealed/**` plus common Bash read
   patterns. This is a guardrail, not enforcement.

[held-out.txt](held-out.txt) is an advisory index of held-out IDs;
`MANIFEST.md` marks them with `H`. The load-bearing integrity check is
`sealed-hashes.txt` verified in CI.

## Harness

The test runner and token counter live in [harness/](harness/) as a
uv-managed Python project.

```bash
cd corpus/harness
uv sync
uv run corpus-run                    # run open tasks only (default)
uv run corpus-run --include-sealed   # Phase 3 grading mode: open + sealed
uv run corpus-run-tacit              # run open Tacit references only
uv run corpus-tokens                 # open-only token counts
uv run corpus-tokens --include-sealed
uv run corpus-verify-sealed          # CI check: sealed/ matches sealed-hashes.txt
uv run corpus-verify-sealed --write  # regen manifest after an ADR-approved sealed edit
```

`corpus-run` compiles each Rust reference with `rustc --edition 2024` into
a tempdir and feeds each test case's `stdin` to both the Python and Rust
binaries, asserting byte-identical `stdout`. `corpus-run-tacit` typechecks,
compiles, and runs each open `reference.tac` against the same tests. Failures
print a diff. The CI workflow in
[.github/workflows/ci.yml](../.github/workflows/ci.yml) runs
`corpus-verify-sealed`, `corpus-run`, and `corpus-run-tacit` on every push
and pull request to `main`.

`corpus-tokens` measures `reference.py`, `reference.tac` when present, and
`reference.rs` with tiktoken's `o200k_base` encoding per
[ADR 0001](../decisions/0001-target-tokenizer.md). Three aggregates are
reported per [ADR 0021](../decisions/0021-corpus-stdlib-dominance-reporting.md):
full, stdlib-dominated, and non-stdlib-dominated. During Stages 4–6, Tacit
aggregates cover the implemented `reference.tac` subset; once all open
references exist, those rows become the full Phase 3 Tacit side. The full and
non-stdlib-dominated aggregates gate Phase 3; the stdlib-dominated aggregate
is reported but not gated. Every task in scope must have an entry in
[stdlib-dominance.toml](stdlib-dominance.toml) or the command errors.

`corpus-verify-sealed` is the load-bearing integrity check per
[ADR 0020](../decisions/0020-sealing-held-out-in-repo.md). It walks
`sealed/`, BLAKE3s every file, and compares to `sealed-hashes.txt`. Any
missing, extra, or mismatched file fails. `--write` regenerates the
manifest from the current tree — only use after an ADR-approved sealed
edit.
