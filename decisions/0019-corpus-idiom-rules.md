# 0019 — Corpus reference-solution idiom rules for Python and Rust

**Status:** Accepted
**Date:** 2026-04-23
**Phase:** 0, Stage 4

## Context

[phase-0-plan.md § Stage 4](../plans/phase-0-plan.md) requires each corpus task to ship with "Reference solutions in Python + Rust" alongside executable test cases. These solutions serve three distinct roles:

1. **Ground-truth correctness.** The references must pass the task's test cases, confirming the task is well-specified.
2. **Token-count baseline for the Phase 3 exit criterion.** [tacit-plan.md](../plans/tacit-plan.md) requires Tacit-Lite to achieve "at least 30% lower end-to-end token usage than equivalent Python." The Python reference *is* the baseline; Rust is a secondary data point.
3. **Primer material source (open subset only).** Phase 3's primer draws Python/Rust ↔ Tacit-Lite pairs from the corpus as progressive examples.

"Equivalent Python" is underspecified. The same task can vary 2–3× in token count depending on idiom:

- Type hints vs no type hints (~15–25% delta on small functions).
- List comprehensions vs explicit loops (~10–30%).
- Standard library (`collections`, `itertools`, `functools`) vs hand-rolled equivalents (~20–40% on the library-using side).
- Code golf vs ordinary readable style (can exceed 50%).
- Docstrings and assertions vs none (~10–20%).

Without a rule, corpus authors — who are the same people authoring the Tacit side of the comparison — have a knob that reaches further than the 30% target itself. The Phase 3 falsification test would then measure author intent rather than the language.

The Rust side carries analogous ambiguity: iterator chains vs imperative loops, `?` vs explicit `match`, pedantic-clean vs default-clean.

## Decision

**Reference solutions are pinned to a single idiomatic style per language. The Python reference is the sole Phase 3 token-count baseline; Rust is a cross-check data point only.**

### Python

- **Formatter:** `ruff format` with project-default config (Black-equivalent). Whitespace is not a design choice.
- **Linter:** `ruff check` clean with the default rule set. No `ALL` rule set — it pushes toward a defensive posture (`TRY`, `PLR`) that doesn't reflect typical Python.
- **Version:** Python 3.12. Modern syntax (pattern matching, `match`/`case`) is permitted where it fits; not required.
- **Type hints:** required on function signatures (parameters and return). Not required on local variables. This reflects both the current centre of idiomatic typed Python and what a modern LLM defaults to producing when asked for Python — which is the relevant baseline, because Phase 3's comparison is against LLM-generated human-language code, not against hand-written or legacy code.
- **Standard library only.** No third-party dependencies. `collections`, `itertools`, `functools`, `dataclasses`, `re`, `math`, `typing` are all fair game.
- **Comprehensions over loops** when the comprehension fits one line or reads cleanly across two. Otherwise use explicit loops. No nesting past two levels.
- **No code golf.** Meaningful names (one-letter names only in comprehensions and math-heavy contexts). No chained ternaries where `if/else` reads better. No `lambda` bound to a name; use `def`.
- **No docstrings on reference solutions.** The task statement is the docstring. Adding one inflates the baseline without reflecting what a model under test would produce.
- **No defensive asserts or argument validation** beyond what the task contract requires. Reference solutions encode correctness; they are not production code.
- **Error handling only where the task contract demands it.** Silent `try/except` blocks are out.

### Rust

- **Edition:** 2024.
- **Formatter:** `cargo fmt` with default config.
- **Linter:** `cargo clippy --all-targets -- -D warnings` clean at the default lint level (no `pedantic`). Matches the CI rule from [ADR 0018](0018-stage-5-frozen.md).
- **Standard library only.** No crates outside `std`.
- **No `unsafe`.**
- **Iterator chains where natural;** `for` loops where they read better. Choice follows clarity, not token count.
- **`Result` and `?` for fallible paths.** No `unwrap()` in reference code except on values unreachable by construction, and only in harness entry points — never in the task solution proper.
- **No doc comments on reference solutions,** mirroring the Python rule.

### Token measurement

- Measured with tiktoken `o200k_base` per [ADR 0001](0001-target-tokenizer.md).
- Scope: the solution source file only. Test-case driver code and harness boilerplate are excluded from both the Python baseline and the Tacit comparison.
- Reported per-task and aggregated. The 30% target is evaluated on the aggregate across the sealed held-out set, not per-task.

### Authorship

LLM assistance in writing reference solutions is explicitly permitted. The Phase 3 comparison target is LLM-generated code in human languages, not hand-written human code — so LLM idiom bias in the Python/Rust references is a feature of the baseline, not a contamination of it. Reviewers apply the rules above regardless of authorship; the rules are what make the baseline comparable across tasks.

### Review process

- Each reference solution is reviewed against these rules at corpus-freeze time.
- Ambiguous algorithm choices (recursive vs iterative for a task that admits both) are resolved by picking the variant an experienced author would reach for first. If two variants are both reasonable and their token counts differ by more than 10%, the *shorter* is chosen — so Tacit is never compared against a strawman.
- Disputes that recur across tasks produce a follow-up ADR amending this one. Individual task judgment calls do not.

## Alternatives considered

- **No style rule ("use your judgment").** Rejected. The 2–3× idiom variance exceeds the 30% target, so the gate would be measuring author intent rather than the language.
- **Code-golf / shortest-possible Python.** Rejected. Not representative of what models write or what humans read. A 30% win over obfuscated one-liners would mean nothing.
- **Production-grade Python (docstrings, asserts, logging).** Rejected. Inflates the baseline artificially in Tacit's favour. Phase 3 is measuring language density, not documentation practice.
- **Multiple Python variants per task, baseline = best.** Rejected. Gives authors a knob, and is inconsistent with the Rust side which will have one variant.
- **Different author for Python than for Tacit.** Rejected as a *mitigation*. Process separation reduces one failure mode but does not remove the idiom ambiguity; the written rule is the safeguard.
- **`ruff` with `ALL` rules on.** Rejected. Pushes code toward a defensive posture that is not how typical Python is written; inflates the baseline in unhelpful directions.
- **Rust at `clippy::pedantic`.** Rejected. Same reasoning as `ruff ALL`; pedantic lints push away from the idiom centre.

## Consequences

- **The 30% Phase 3 target becomes a measurable property**, not a negotiable judgment. If Tacit doesn't hit 30% against these baselines, the thesis is refuted under its own rules.
- **Corpus authoring is mechanical rather than stylistic.** Stage 4 gets a checklist; idiom disputes become ADRs rather than drift.
- **Primer Python examples are consistent.** The Phase 3 model sees one "flavour" of Python, which should transfer more cleanly than a mixed diet.
- **Some tasks will feel unnaturally constrained.** A task where a 20-line stdlib Python solution could be a 3-line numpy call will penalise Python artificially. Accepted — the corpus is calibrated on Tacit-Lite's standard-library slice, not the full Python ecosystem. "Numpy-parity" is a Phase 5 concern if it arises.
- **Baseline re-measurement is cheap.** `ruff format && tiktoken-count` is scriptable. If tokenizer access changes (see ADR 0001's reopener), re-measurement is a one-line swap.
- **Rust baseline is not load-bearing.** A Rust number that disagrees with Python doesn't fail Phase 3; it produces a data point the decision log can explore. Deliberate: one authoritative baseline, not a two-gate composite.
- **These rules freeze with the corpus at Stage 4 exit.** Changes require a new ADR, matching the Stage 2 and Stage 3 freeze discipline.

## Related decisions

- [ADR 0001](0001-target-tokenizer.md) — target tokenizer; supplies the measurement tool and the reopener for Claude-tokenizer re-validation.
- [ADR 0018](0018-stage-5-frozen.md) — sets the Rust lint level this ADR reuses for reference solutions.
- [phase-0-plan.md § Stage 4](../plans/phase-0-plan.md) — the deliverable this ADR scopes.
- [tacit-plan.md § Phase 3 exit criteria](../plans/tacit-plan.md) — the 30% target this ADR protects.
