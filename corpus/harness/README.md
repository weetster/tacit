# corpus/harness

Python test-runner and token-counter for the Phase 3 evaluation corpus.

## Setup

```bash
cd corpus/harness
uv sync
```

Requires `rustc` on `$PATH` (for the Rust reference solutions — Stage 4
does not yet require Cargo, since each `reference.rs` is self-contained).
`corpus-run-tacit` requires a built `tacit` CLI with LLVM support; by
default it uses `$TACIT_BIN`, `../../target/debug/tacit`, or `tacit` on
`$PATH`.

## Commands

```bash
uv run corpus-run        # compile + run Python/Rust references
uv run corpus-run-tacit  # compile + run open Tacit references
uv run corpus-tokens     # print per-task tiktoken o200k_base counts
uv run corpus-preflight  # local preflight before any paid corpus-eval run
uv run corpus-eval --dry-run
```

`corpus-run` exits non-zero if any test fails. It prints a dot per passing
test, `F` per failing test, then a summary with any failure details.
`corpus-run-tacit` has the same output shape and intentionally runs only
open tasks with `reference.tac` files.

`corpus-tokens` reports `reference.py`, `reference.tac` when present,
`reference.stdlib.tac` when present, and `reference.rs` token counts per task.
It prints aggregate deltas for the implemented core-language Tacit references,
the stdlib-mediated Tacit references, and the paired stdlib-vs-current Tacit
subset. The aggregate Python count is the Phase 3 baseline per
[ADR 0019](../../decisions/0019-corpus-idiom-rules.md).

`corpus-preflight` runs locally with the same `--tacit-bin` that a paid
`corpus-eval` will use. It checks and compiles tiny smoke programs for
`@token-index-any`, `@sort-i64`, `@sort-ranges-by-bytes`,
`@count-equal-ranges`, and `@dedup-adjacent-ranges`; runs the 12 stdlib
canary `reference.stdlib.tac` files against their `tests.jsonl`; reports the
canary subset's stdlib-Tacit token total against current Tacit and Python
references; and prints the binary path, mtime, and BLAKE3 hash. Pass `--json`
for machine-readable output. Exits non-zero if any smoke or canary check
fails. Run this before any paid stdlib-mediated rerun.

`corpus-eval` drives the Phase 3 model-generation loop and writes paired
`<run-id>.run.json` / `<run-id>.metrics.json` files under
`../../plans/phase-3-results/` by default. Real Anthropic runs read
`ANTHROPIC_API_KEY`; OpenRouter runs read `OPENROUTER_API_KEY`. Each
`run.json` records the `tacit_binary` path, mtime, and BLAKE3 so source/binary
skew is visible after the fact.

```bash
uv run corpus-eval --model claude-sonnet-4-6 --tasks 001
uv run corpus-eval --model claude-sonnet-4-6 --repair-turns 2 --tasks 033,035,037
uv run corpus-eval --model claude-sonnet-4-6 --result-label library-mediated --tasks 025,035
uv run corpus-eval --provider openrouter --model openai/gpt-5.5 --tasks 001
uv run corpus-eval --model claude-sonnet-4-6 --include-sealed
```

The default scope is open tasks only. `--include-sealed` switches to the sealed
held-out scope for grading runs; the harness records task IDs but never writes
sealed task statements into metrics. `--dry-run` uses open `reference.tac`
files as synthetic model outputs and makes no API calls. `--repair-turns`
defaults to `0`; values up to `2` enable the open-only repair protocol, adding
per-turn repair fields and final-pass aggregates while preserving the existing
one-shot fields for turn 0.

Use `--result-label library-mediated` for stdlib-mediated experiments. The
label is written to both `run.json` and `metrics.json`; primary Phase 3 gates
are marked reporting-only for these runs so they cannot satisfy the
primer-only core-language gate.
