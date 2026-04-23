# 0021 — Corpus stdlib-dominance reporting for Phase 3 token baseline

**Status:** Accepted
**Date:** 2026-04-23
**Phase:** 0, Stage 4

## Context

[ADR 0019](0019-corpus-idiom-rules.md) pins Python reference solutions to idiomatic stdlib-using style. The rationale is sound: stdlib is what an LLM produces when asked for Python, and "shorter variant wins" prevents strawman baselines.

That decision has an unexamined consequence. A non-trivial slice of the corpus reduces to a single stdlib call on the Python side — `bisect_left` for binary search, `Counter` for frequency counts, `heapq.nlargest` for top-k, `re.findall` for tokenisation, `itertools.groupby` for run-length work, and so on. On those tasks the Python baseline reflects the maturity of Python's standard library — 30+ years of curation — rather than the density of the language itself.

Tacit-Lite as evaluated in Phase 3 ships with essentially no standard library. On stdlib-dominated tasks, the Tacit solution will hand-roll primitives that the Python reference delegates. The 30% token-reduction target, evaluated as a single aggregate across the held-out set, therefore measures a mixture of two things:

1. Language-level density (what the Phase 3 thesis actually claims).
2. Standard-library parity (not claimed, and out of scope until Phase 5+).

The [ADR 0019 § Consequences](0019-corpus-idiom-rules.md) block addresses the *opposite* direction — that Python tasks excluding numpy may feel constrained — but does not address the direction that matters here: that *including* Python's stdlib may make the aggregate a partial proxy for ecosystem maturity.

Two fixes were considered for the underlying idiom rule and rejected earlier:

- **Hand-roll the Python reference** on stdlib-dominated tasks. Rejected as a strawman under 0019's rule 68. Not revisited here.
- **Defer Phase 3 until Tacit has a comparable stdlib.** Rejected as unfalsifiable — a gate that can always be deferred is not a gate.

The remaining path is to leave 0019's rule untouched and change what is *reported*, so that a near-miss on the 30% aggregate can be diagnosed rather than collapsed into a single pass/fail.

## Decision

**The corpus grows one metadata field and the token-count harness reports three aggregates instead of one. ADR 0019's idiom rule is not modified.**

### `stdlib_dominated` tag

Each task gains a boolean `stdlib_dominated` field, stored in a single top-level index file (`corpus/stdlib-dominance.toml`) rather than in per-task `task.md` frontmatter. One file keeps the tagging auditable in a single diff and keeps `task.md` as prose.

The field is fixed at Stage 4 freeze and can only be changed by a subsequent ADR — the same discipline as the idiom rules themselves, for the same reason: authors cannot retune it to move the aggregate.

**Sealed tasks.** Entries for held-out tasks live in the same central file. Per [ADR 0020](0020-sealing-held-out-in-repo.md) the sealed IDs are already public in `held-out.txt`, so a boolean per known ID adds no material leak. The sealed-task author writes the entry as part of the same PR that seals the task, and `sealed-hashes.txt` is not affected because `stdlib-dominance.toml` lives outside `sealed/`. `corpus-verify-sealed` is unchanged.

### Tagging rule

A task is `stdlib_dominated = true` iff **rewriting the Python reference without any non-I/O stdlib import would add ≥ 5 non-blank, non-import lines to the solution body.**

Mechanical definition:

- **Non-I/O stdlib imports** are all imports except `sys`, `io`, and `typing`. Imports used purely for type hints (`typing.Callable` etc.) do not count toward stdlib dominance.
- **The I/O exclusion is narrow by design.** It covers modules that merely wrap what the operating system or language type system already provides — moving bytes across the stdin/stdout boundary, or annotating types. Everything *algorithmic* — parsing data formats, searching, sorting, data structures, regex, compression, hashing — counts toward stdlib dominance even when the module sits near an I/O boundary. Concretely: `json`, `csv`, `re`, `struct`, `pickle`, `base64`, `hashlib`, `zlib` are all algorithmic stdlib under this rule. JSON parsing is not trivially provided by libc the way `read`/`write` are; mature-stdlib advantage is exactly what this ADR is measuring, and carving out `json` would hide that advantage.
- **Solution body** excludes `main()` I/O boilerplate (reading stdin, splitting fields, printing results) and excludes import statements themselves.
- The rewrite is a thought experiment at tagging time, not an executed artifact. The reviewer imagines the hand-rolled version and counts lines.
- Ties (exactly 4 or 5 added lines) default to `false`. Tagging is conservative — only clear cases get flagged, to keep the non-stdlib-dominated aggregate strict.

Examples of the rule in action:

- `031-binary-search` uses `bisect_left`. Hand-rolled binary search is 6–8 lines. **Tagged `true`.**
- A hypothetical word-frequency task using `collections.Counter`. Hand-rolled `dict` accumulation is 4–5 lines. **Borderline; tagged `false`** under the tie-goes-to-false rule.
- A sum-of-squares task with no imports. **Tagged `false`** trivially.
- A JSON-parsing task using `json.loads`. **Tagged `true`** — `json` is algorithmic stdlib under the libc-principle above, not I/O.

### Three reported aggregates

`corpus-tokens` reports, per language:

1. **Full aggregate** — all tasks in scope.
2. **Stdlib-dominated aggregate** — tasks with `stdlib_dominated = true`.
3. **Non-stdlib-dominated aggregate** — tasks with `stdlib_dominated = false`.

Each aggregate is the sum of Python (and, separately, Rust) token counts over its member tasks, with the Tacit comparison computed against each.

### Phase 3 pass condition

Phase 3's 30% exit criterion is evaluated against **both** the full aggregate **and** the non-stdlib-dominated aggregate. Passing only the full aggregate is reported as a qualified pass ("carried by stdlib-dominated tasks"); passing only the non-stdlib-dominated aggregate is a qualified pass in the other direction. Passing both is an unqualified pass. Failing both fails Phase 3.

The stdlib-dominated aggregate is reported but not gated — Tacit is expected to lose ground there until it has a comparable standard library, and failing that aggregate alone does not fail Phase 3.

## Alternatives considered

- **Do nothing; keep the single aggregate.** Rejected. A near-miss becomes undiagnosable, and a pass carried by a 60/40 mix of stdlib-dominated losses and language wins is reported the same as a clean language win. The thesis claim deserves a cleaner signal.
- **Remove stdlib-dominated tasks from the corpus.** Rejected. Those tasks are real programming work; excluding them biases the corpus toward algorithms-from-scratch, which is a different (and narrower) evaluation.
- **Tag in per-task `task.md` frontmatter.** Rejected. Splits the tagging surface across 60 files, making audit and re-tagging diffs noisier. One index file is the simpler load-bearing artifact.
- **Executable tagging (strip imports, count delta via a lint pass).** Rejected. Brittle — not all stdlib uses can be mechanically stripped, and `collections.defaultdict`-style idioms resist automated rewriting. The reviewer thought-experiment is more honest about the judgment being made.
- **Graduated tag (`low` / `medium` / `high` stdlib dominance) instead of boolean.** Rejected as over-engineering. Two buckets give the diagnostic signal; three or more invite debate without sharpening the result.

## Consequences

- **Phase 3 near-miss becomes diagnosable.** If Tacit hits 28% on the full aggregate but 34% on the non-stdlib-dominated subset, the written record is "language-level density target met; stdlib parity remains a Phase 5+ concern" rather than "thesis refuted."
- **The thesis claim is honestly scoped.** The 30% number is no longer ambient — it attaches to a specific aggregate and a specific subset. Anyone reading the Phase 3 report can see which of the two was load-bearing.
- **Tagging is a one-time Stage 4 cost.** ~60 tasks × one reviewer thought-experiment each; trivial compared to corpus authoring itself.
- **The tagging rule is audit-able but not mechanical.** The ≥ 5-line threshold is a reviewer judgment, not an executable check. Disputes get logged in the PR; recurring disputes produce a follow-up ADR amending the threshold.
- **`corpus-tokens` output grows.** Three aggregates per language instead of one. Consumers parsing the output must be updated; there are no such consumers yet, so the cost is future-paid.
- **This ADR freezes with the corpus at Stage 4 exit.** Same discipline as ADR 0019 — tag changes and rule changes both require a new ADR post-freeze.

## Related decisions

- [ADR 0019](0019-corpus-idiom-rules.md) — the idiom rule this ADR layers reporting onto. Not modified.
- [ADR 0001](0001-target-tokenizer.md) — tokenizer used by `corpus-tokens`.
- [ADR 0020](0020-sealing-held-out-in-repo.md) — the sealed-hashes discipline this ADR extends to `stdlib-dominance.toml`, which must be included in the sealed-files manifest where it covers sealed tasks.
- [phase-0-plan.md § Stage 4](../plans/phase-0-plan.md) — the deliverable this ADR scopes.
- [tacit-plan.md § Phase 3 exit criteria](../plans/tacit-plan.md) — the 30% target this ADR sharpens.
