# Evaluation Harness Runbook

**Status:** Operational instructions
**Date:** 2026-05-04

This runbook is for agents or lower-capability models that need to run the
Phase 3 evaluation harness correctly on the first try.

## Hard Rule: Sealed Corpus

Do not read, list, search, grep, inspect, summarize, or otherwise access
anything under `corpus/sealed/` unless the user explicitly instructs you to run
a sealed grading command. Never discover sealed task names by listing the
sealed tree.

By default, harness commands use open tasks only. `--include-sealed` is the
only normal switch that brings the sealed held-out scope into these commands.
Use it only when the user explicitly asks for sealed or held-out grading.

When sealed mode is requested:

- Do not inspect `corpus/sealed/` manually.
- Do not tune code, prompts, or plans from sealed task contents or failures.
- Use only selectors the user gave you, or run the full sealed scope if the
  user explicitly asked for a full sealed grading run.
- Do not use `--repair-turns`; the harness rejects sealed repair runs.
- Do not use `--retain-outputs`; the harness rejects retained outputs for
  sealed runs.
- Treat sealed outputs as grading artifacts, not development feedback.

`corpus-verify-sealed` intentionally walks `corpus/sealed/` for CI hash
verification. Do not run it manually unless the user explicitly asks for sealed
manifest verification. Do not run `corpus-verify-sealed --write` unless the
user explicitly asks to regenerate the sealed manifest after an approved sealed
corpus change.

## Agent Process Discipline

Run harness commands from `corpus/harness` with `uv run`.

Do not poll the harness process for output until it exits. Start the command
with a long enough timeout and wait for completion. The harness prints progress
dots and writes final JSON artifacts at the end; partial progress output is not
actionable and can lead to confused summaries.

For a long paid run, wait for the process to exit before reporting results. If
your execution tool returns a live session, wait for final completion instead
of repeatedly polling for incremental output.

## Prerequisites

From the repository root, build the Tacit CLI when a command needs
`corpus-run-tacit`, `corpus-preflight`, or `corpus-eval`:

```bash
cargo build --features tacit-codegen/llvm19-1,tacit-cli/llvm19-1
```

The harness finds the Tacit binary in this order:

1. `TACIT_BIN`
2. `../../target/debug/tacit` relative to `corpus/harness`
3. `tacit` on `PATH`

`corpus-run` also needs `rustc` on `PATH` or at `~/.cargo/bin/rustc`.

Real `corpus-eval` runs need an API key:

- Anthropic provider: `ANTHROPIC_API_KEY`
- OpenRouter provider: `OPENROUTER_API_KEY`

The evaluator loads a `.env` file from the current working directory. This is
another reason to run from `corpus/harness`.

In practice, that means `corpus/harness/.env` is loaded automatically when you
run the harness from `corpus/harness` with `uv run`. If you run the command from
another directory, the harness will look for `.env` in that directory instead,
so prefer `cd corpus/harness` first or export the key explicitly.

## Setup

```bash
cd corpus/harness
uv sync
uv run corpus-eval --help
```

All remaining `uv run` examples assume the current directory is
`corpus/harness`.

## Local No-API Checks

Run the Python and Rust reference solutions for open tasks:

```bash
uv run corpus-run
```

Run open Tacit references with the built Tacit CLI:

```bash
uv run corpus-run-tacit --tacit-bin ../../target/debug/tacit
```

Count open-reference tokens:

```bash
uv run corpus-tokens
```

Run the local preflight before any paid eval that will use the same Tacit
binary:

```bash
uv run corpus-preflight --tacit-bin ../../target/debug/tacit
```

Use JSON preflight output only when a script needs it:

```bash
uv run corpus-preflight --tacit-bin ../../target/debug/tacit --json
```

Exercise the eval pipeline without API calls:

```bash
uv run corpus-eval --dry-run --tasks 001 --tacit-bin ../../target/debug/tacit
```

`--dry-run` uses open `reference.taca` files as synthetic model output (the
preserved authoring-view text from the Phase 3 corpus, paired with the
canonical `reference.tac`) and makes no API calls. If `--dry-run` has no
`--tasks` selector, the harness runs only the first open task.

## Running Specific Tasks

Use `--tasks` with `corpus-eval`. Selectors can be:

- Number only: `001`
- Task directory name: `001-sum-to-n`
- Family plus task directory: `arithmetic/001-sum-to-n`

Comma-separated selectors and repeated flags are both valid:

```bash
uv run corpus-eval --dry-run --tasks 001,033 --tacit-bin ../../target/debug/tacit
uv run corpus-eval --dry-run --tasks 001 --tasks algorithms/033-two-sum --tacit-bin ../../target/debug/tacit
```

Selectors apply to the current scope. Without `--include-sealed`, they match
open tasks. With `--include-sealed`, they match sealed tasks. Do not list the
sealed tree to find sealed selectors.

## Real Eval Recipes

Always run preflight first with the exact Tacit binary you will pass to
`corpus-eval`:

```bash
uv run corpus-preflight --tacit-bin ../../target/debug/tacit
```

Run one open task through Anthropic:

```bash
uv run corpus-eval --model claude-sonnet-4-6 --tasks 001 --tacit-bin ../../target/debug/tacit
```

Run a small open repair experiment:

```bash
uv run corpus-eval --model claude-sonnet-4-6 --repair-turns 2 --tasks 033,035,037 --tacit-bin ../../target/debug/tacit
```

Run an OpenRouter model:

```bash
uv run corpus-eval --provider openrouter --model openai/gpt-5.5 --tasks 001 --tacit-bin ../../target/debug/tacit
```

Run a library-mediated experiment:

```bash
uv run corpus-eval --model claude-sonnet-4-6 --result-label library-mediated --tasks 025,035 --tacit-bin ../../target/debug/tacit
```

Run sealed grading only when explicitly instructed:

```bash
uv run corpus-eval --model claude-sonnet-4-6 --include-sealed --tacit-bin ../../target/debug/tacit
```

Do not add `--repair-turns` or `--retain-outputs` to sealed runs.

## `corpus-eval` Arguments

`--model MODEL`
: Model id. Required for real runs. Optional for `--dry-run`.

`--provider {auto,anthropic,openrouter}`
: Provider. Default `auto` uses Anthropic for model ids starting with
`claude-`; otherwise it uses OpenRouter.

`--track {auto,primary,cross-family}`
: Metrics track. Default `auto` chooses `primary` for Anthropic and
`cross-family` for OpenRouter.

`--result-label {core-language,library-mediated}`
: Result interpretation label. Default `core-language`. Use
`library-mediated` for stdlib-mediated experiments; those runs are reported
separately and do not satisfy primary Phase 3 gates.

`--include-sealed`
: Switch from open tasks to sealed held-out tasks. Use only when explicitly
instructed.

`--tasks TASKS`
: Task selector list. Accepts comma-separated values and can be repeated.

`--output-dir OUTPUT_DIR`
: Directory for `<run-id>.run.json`, `<run-id>.metrics.json`, and `failures/`.
Default is `../../plans/phase-3-results/`.

`--tacit-bin TACIT_BIN`
: Path to the Tacit CLI binary. Prefer passing
`--tacit-bin ../../target/debug/tacit` so the run records the intended binary.

`--dry-run`
: No API call. Uses open Tacit references as synthetic model outputs.

`--retain-outputs`
: Writes raw model outputs under `<run-id>.outputs/` for open tasks. Disabled
for sealed runs.

`--repair-turns N`
: Number of repair turns after initial generation. Valid range is `0` to `2`.
Default is `0`. Open-only.

`--max-tokens N`
: Model max output tokens. Default is `8192`.

`--temperature FLOAT`
: Sampling temperature. Default is `0`.

`--timeout-seconds N`
: Per API request timeout. Default is `120`.

`--max-retries N`
: Retries for transient API failures. Default is `3`.

## Other Harness Commands

`uv run corpus-run`
: Runs Python and Rust references for open tasks. Add `--include-sealed` only
when the user explicitly asks for sealed reference validation.

`uv run corpus-run-tacit --tacit-bin ../../target/debug/tacit`
: Runs open Tacit references only. This command has no sealed mode.

`uv run corpus-tokens`
: Counts open reference tokens. Add `--include-sealed` only when explicitly
asked to count sealed tasks.

`uv run corpus-preflight --tacit-bin ../../target/debug/tacit`
: Checks the Tacit binary, compiles smoke programs for stdlib primitives, runs
the stdlib canary references, reports token totals, and prints binary metadata.
Run this before every paid eval.

`uv run corpus-verify-sealed`
: Verifies the sealed manifest. Run only when explicitly instructed because it
walks `corpus/sealed/`.

## Outputs To Report

After `corpus-eval` exits, report:

- Exit code.
- `run_id` printed by the harness.
- Paths to `<run-id>.run.json` and `<run-id>.metrics.json`.
- Final aggregate line, such as `tasks_passed`, `compile_pass`, and
  `token_delta`, or repair-loop aggregate values.
- Any failure directories the harness wrote under the output directory.
  Failure records contain `generated.taca` (the model's authoring-view output)
  and `diagnostics.json`. They do not contain a `generated.tac`; use
  `tacit canonicalize generated.taca` to produce a canonical form if needed.

Do not summarize partial dot progress. Wait for the final printed summary.

## Common Failures

`tacit binary not found`
: Build the CLI or pass `--tacit-bin ../../target/debug/tacit`.

`ANTHROPIC_API_KEY must be set` or `OPENROUTER_API_KEY must be set`
: Set the provider key in the environment or in `corpus/harness/.env`, then
rerun from `corpus/harness`.

`rustc not found`
: Install Rust or put `rustc` on `PATH`. This affects `corpus-run`.

`no tasks matched selector(s)`
: The selector does not match the current scope. Check whether the run is open
or sealed, but do not inspect `corpus/sealed/`.

`--repair-turns is open-only`
: Remove `--repair-turns` or switch to open scope.

`--retain-outputs is disabled for sealed runs`
: Remove `--retain-outputs`. Sealed raw outputs must not be retained.

Paid run is unexpectedly large
: Stop before making a full paid corpus call unless the user requested it. Run
a single open task or `--dry-run` first.
