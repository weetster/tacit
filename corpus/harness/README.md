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
```

`corpus-run` exits non-zero if any test fails. It prints a dot per passing
test, `F` per failing test, then a summary with any failure details.
`corpus-run-tacit` has the same output shape and intentionally runs only
open tasks with `reference.tac` files.

`corpus-tokens` reports `reference.py`, `reference.tac` when present, and
`reference.rs` token counts per task and aggregate deltas over the implemented
Tacit references. The aggregate Python count is the Phase 3 baseline per
[ADR 0019](../../decisions/0019-corpus-idiom-rules.md).
