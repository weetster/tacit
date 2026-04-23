# corpus/harness

Python test-runner and token-counter for the Phase 3 evaluation corpus.

## Setup

```bash
cd corpus/harness
uv sync
```

Requires `rustc` on `$PATH` (for the Rust reference solutions — Stage 4
does not yet require Cargo, since each `reference.rs` is self-contained).

## Commands

```bash
uv run corpus-run       # compile + run both references against tests.jsonl
uv run corpus-tokens    # print per-task tiktoken o200k_base counts
```

`corpus-run` exits non-zero if any test fails. It prints a dot per passing
test, `F` per failing test, then a summary with any failure details.

`corpus-tokens` reports `reference.py` and `reference.rs` token counts per
task and the aggregate. The aggregate Python count is the Phase 3 baseline
per [ADR 0019](../../decisions/0019-corpus-idiom-rules.md).
