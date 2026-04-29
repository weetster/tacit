# 0020 — Seal held-out corpus tasks in-repo via multi-layer guardrails

**Status:** Accepted
**Date:** 2026-04-23
**Phase:** 0, Stage 4

## Context

[phase-0-plan.md § Stage 4](../plans/phase-0-plan.md) originally specified
storing the held-out subset's hashes in a **separate repository** to enforce
tamper-detection:

> Seal ~20% as held-out; store hashes of held-out set in a separate repo to enforce

The intent is two-fold:

1. **Contamination prevention.** Held-out tasks must not appear in primer
   examples, training material, or anything the Phase 3 model-under-test
   sees. Physical separation was one way to enforce this.
2. **Tamper detection.** If someone edits a held-out test to match a
   passing Tacit output, Phase 3 would silently grade easier. Hashes stored
   out-of-band let an auditor recompute and compare.

A separate repo delivers both via physical isolation, but at the cost of
two git histories, a second clone in CI, a release-time coordination step
at Stage 4 freeze, and out-of-band credentials to write to the archive
repo. For a Phase 0 artifact with two held-out tasks today (002, 022) and
twelve planned, that overhead looks disproportionate.

Both properties can be layered within this repo if the guardrails are
layered — and they should be layered either way, because *neither* path
(separate repo or in-repo) is self-enforcing: a separate repo doesn't stop
an author from manually copying held-out content into a primer, and an
in-repo `sealed/` directory doesn't stop a walker script that ignores the
convention. Enforcement is always procedural; the question is whether the
procedures are plausibly followed.

## Decision

**Held-out tasks live at `corpus/sealed/<category>/<NNN-slug>/` in this
repo, mirroring the open-tasks layout under `corpus/tasks/`.** The
tamper-detection guarantee comes from `corpus/sealed-hashes.txt` (BLAKE3
per file, sorted by path) verified in CI, not from physical separation.

Four guardrails layer on top:

### Layer 1 — Directory separation

Sealed tasks are never at the same path as open tasks. Every tool that
walks the corpus must decide explicitly whether to include sealed. Default
is *exclude*.

### Layer 2 — Hash manifest + CI verification

`corpus/sealed-hashes.txt` records a BLAKE3 of every file under
`corpus/sealed/`. The harness command `corpus-verify-sealed` walks
`sealed/`, re-hashes, and fails if any file is missing, extra, or
modified.

The Stage 4 freeze commits this file. Post-freeze, any change to a sealed
file must:

- be proposed by an ADR amending or superseding this one,
- accompany a regenerated `sealed-hashes.txt`,
- be reviewed in a PR where the diff to `sealed/` is visible.

CI runs `corpus-verify-sealed` on every push. A PR that edits `sealed/`
without regenerating the manifest fails CI. A PR that edits both is
still visible to reviewers precisely because the manifest diff is large
and obvious.

BLAKE3 is reused here from [ADR 0009](0009-hashing-rule.md); no second
hash primitive is introduced.

### Layer 3 — Harness flag with *exclude* as default

- `uv run corpus-run` — open tasks only.
- `uv run corpus-run --include-sealed` — open + sealed.
- `uv run corpus-tokens` — open tasks only.
- `uv run corpus-tokens --include-sealed` — both.

Any future primer-building or training-corpus-producing tool must follow
the same convention: *sealed is off by default, opt-in for grading only.*
A tool that walks `corpus/tasks/` sees only open tasks; one that walks
`corpus/` sees both. This makes "do I include held-out?" a deliberate
question the author must answer, rather than an ambient property of the
filesystem.

### Layer 4 — AI agent guardrails

Two agent-specific files block the easy paths to reading held-out content:

- **`.claude/settings.json`** denies Read/Edit/Write on `corpus/sealed/**`
  plus common Bash read patterns (`cat`, `less`, `head`, `tail`, `bat`,
  `grep`, `rg`, `find`). Binds Claude Code sessions in this repo's cwd.
- **`AGENTS.md`** instructs OpenAI Codex CLI (and any other agent that
  reads `AGENTS.md`) never to read, list, search, or otherwise access any
  path under `corpus/sealed/`.

This is the *softest* of the four layers — it does not protect against
`--dangerously-skip-permissions`, agents that ignore project instructions,
or human inspection — and is explicitly not load-bearing. It exists to make
the easy path the right path for AI coding agents working in this repo.

## Alternatives considered

- **Separate repo (as originally planned).** Rejected for Stage 4 as
  disproportionate to the artifact size; could be adopted later if the
  project attracts public contributors and tamper risk rises. Migration
  cost is low: `git filter-repo` over `corpus/sealed/` plus a submodule
  pointer would convert either direction.

- **Encrypted sealed files.** Rejected. The harness needs plaintext at
  grading time, so the encryption key would have to live alongside the
  ciphertext (or be held by one person), which degrades to either "not
  encrypted" or "one person can grade". Neither helps.

- **Claude permission denies alone.** Rejected as the sole mechanism.
  Permission denies are soft, agent-specific, and bypassable. Without the
  hash manifest, no one would detect a held-out test being quietly edited.

- **Hash manifest without Claude denies.** Rejected as insufficiently
  layered given the guideline in this ADR that "neither path is
  self-enforcing." Adding the deny rules costs one JSON file and removes
  one obvious failure mode.

- **One flat `held-out/` directory without category subdirs.** Rejected.
  Mirroring the category structure keeps task IDs stable and makes "move
  an open task into held-out" a simple `git mv`, not a re-slug.

- **Include sealed by default, flag to exclude.** Rejected. The exclude
  default *is* the guardrail; flipping it inverts the risk model so that
  forgetting a flag leaks content into training data.

## Consequences

- **Stage 4 exit no longer requires a second repo.** One `git add` of
  `corpus/sealed-hashes.txt` at freeze is sufficient. The hash manifest
  itself is the archival artifact.
- **Every tool that walks the corpus must decide about sealed.** The
  harness now has a flag; future tools (primer generator, Phase 3
  eval script, training-data exporter) inherit the same default-exclude
  convention. This ADR is the reference for that convention.
- **Guardrails must all stay working to be credible.** If CI stops
  running `corpus-verify-sealed`, Layer 2 becomes advisory and the sealing
  silently degrades. The CI job is load-bearing and should never be
  skipped — [ADR 0018](0018-stage-5-frozen.md) covers the existing CI
  workflow; a follow-up to that workflow adds this check.
- **[phase-0-plan.md § Stage 4](../plans/phase-0-plan.md) is superseded
  in the "separate repo" clause only.** Everything else in the Stage 4
  spec is unchanged; the pointer from the plan to this ADR is updated.
- **Migration to a separate repo remains possible** without breaking any
  hashes, task paths, or harness invocations — the manifest is the
  portable artifact.
- **Reviewers of PRs touching `sealed/` must read diffs carefully.** The
  manifest changing is a strong signal; prose review of the task contract
  still matters.

## Related decisions

- [ADR 0009](0009-hashing-rule.md) — BLAKE3, the hash reused here.
- [ADR 0018](0018-stage-5-frozen.md) — CI workflow that gains the
  `corpus-verify-sealed` step.
- [ADR 0019](0019-corpus-idiom-rules.md) — idiom rules whose held-out
  subset is what this ADR seals.
- [phase-0-plan.md § Stage 4](../plans/phase-0-plan.md) — the Stage 4
  deliverable whose "separate repo" clause this ADR supersedes.
