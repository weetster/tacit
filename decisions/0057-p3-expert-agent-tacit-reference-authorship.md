# 0057 — Expert-agent authorship for Tacit-Lite corpus references

**Status:** Accepted
**Date:** 2026-04-30
**Phase:** 3, Stage 4
**Amends:** [ADR 0048](0048-p3-tacit-idiom-rules.md)
**Consults:** [ADR 0020](0020-sealing-held-out-in-repo.md), [ADR 0049](0049-p3-examples-layout-contamination.md), [ADR 0051](0051-p3-tacit-token-rule.md), [ADR 0052](0052-p3-eval-model-contract.md)

## Context

[ADR 0048](0048-p3-tacit-idiom-rules.md) forbids LLM assistance when
writing Tacit-Lite corpus references. The intent was sound: prevent the
Phase 3 benchmark from collapsing into "one model imitates another model's
answer key" and keep the Tacit side of the token comparison from being
gamed by prompt-specific quirks.

That rule creates two practical and conceptual problems:

1. **Tacit is AI-first.** The project thesis is not that humans should
   comfortably hand-write Tacit-Lite; it is that models can write Tacit
   well when given a compact, precise representation and the right primer.
   Requiring unaided human authorship for all `reference.tac` files is
   therefore misaligned with the language's intended authoring mode.
2. **The reference role is feasibility, not eval context.** The model
   under test never receives `reference.tac`; per
   [ADR 0052](0052-p3-eval-model-contract.md), it receives only the
   primer and the task statement. A high-reasoning expert agent with full
   repository and ADR context can produce reference implementations that
   establish "this task is expressible in Tacit-Lite" without giving the
   evaluated model a copy path.

The real contamination boundary is already pinned elsewhere:

- [ADR 0020](0020-sealing-held-out-in-repo.md) forbids reading or using
  sealed held-out material during development.
- [ADR 0049](0049-p3-examples-layout-contamination.md) forbids the
  primer from drawing verbatim examples from `corpus/tasks/**/reference.tac`.
- [ADR 0052](0052-p3-eval-model-contract.md) forbids `reference.py`,
  `reference.tac`, tests, and prior-task history from the model prompt.

Those boundaries are sufficient to prevent answer-key leakage. The author
identity of `reference.tac` is less important than whether references are
produced under a reproducible expert-authorship contract and reviewed
against [ADR 0048](0048-p3-tacit-idiom-rules.md)'s idiom rules.

## Decision

**Tacit-Lite corpus references may be authored by a high-reasoning expert
agent with full access to the open repository context and ADRs. ADR 0048's
"LLM assistance is not permitted" rule is superseded by this ADR.**

The replacement authorship rule is:

- `reference.tac` and `reference.tac.sidecar.toml` are **expert-agent
  authored**, not unaided-human authored.
- The authoring agent may inspect open corpus tasks under `corpus/tasks/`,
  existing examples, compiler code, docs, plans, and ADRs.
- The authoring agent must not read, list, search, derive from, or otherwise
  access `corpus/sealed/`, matching [ADR 0020](0020-sealing-held-out-in-repo.md).
- The authoring agent may use tests for the open task being implemented;
  this is reference construction, not model evaluation.
- The authoring agent may run the compiler, typechecker, token counter, and
  harness locally to converge on a correct reference.

The references remain governed by all non-authorship rules in ADR 0048:

- authoring-view source plus sidecar in each open task directory;
- no references for sealed tasks;
- no comments in `reference.tac`;
- no primitive aliasing;
- monomorphic concrete effects by default;
- algorithmic shape follows the Python reference unless Tacit has a clearly
  more direct expression;
- `rec` is used for self-recursion or mutual recursion rather than manual
  unrolling;
- each reference must compile, typecheck against its sidecar effect
  annotation, and pass every `tests.jsonl` case.

### Authorship Manifest

Stage 4 introduces an authorship manifest at:

```
corpus/tacit-reference-authorship.toml
```

The manifest records one entry per Tacit reference:

```toml
[[reference]]
task = "strings/013-is-palindrome"
path = "corpus/tasks/strings/013-is-palindrome/reference.tac"
authoring_mode = "expert-agent"
agent = "codex"
model = "gpt-5.x"
date = "2026-04-30"
sealed_access = false
```

The exact `model` string is whatever the host environment exposes at the
time of authorship. If the environment does not expose a stable model ID,
the value is `"undisclosed"` and `agent` remains the stable tool-family
label. The load-bearing fields are `authoring_mode = "expert-agent"` and
`sealed_access = false`.

The manifest is descriptive, not part of the compiler or harness input.
It exists so later readers can distinguish expert-agent references from
human-only references and can audit which tasks were generated under this
amended authorship rule.

### Evaluation Boundary

This ADR does **not** change the Phase 3 evaluation prompt. The evaluated
model still receives only:

1. `plans/primer/tacit-lite-primer.md`
2. the task statement

It does not receive:

- `reference.tac`;
- `reference.py`;
- `reference.rs`;
- `tests.jsonl`;
- the authorship manifest;
- prior generated solutions;
- harness feedback from earlier attempts.

The eval remains "can Sonnet/Haiku write passing Tacit-Lite from the
primer alone?", not "can they reproduce the expert-agent reference?"

### Primer Boundary

[ADR 0049](0049-p3-examples-layout-contamination.md) remains unchanged.
The primer still may not paste or near-paste `corpus/tasks/**/reference.tac`.
Expert-agent authorship of references does not make those references primer
source material. The primer draws from `examples/smoke/`,
`examples/phase-3/`, and fresh snippets only.

### Review Standard

The review question changes from "was this written unaided by a human?" to
"does this reference represent what an expert Tacit agent should write under
the frozen ADR surface?"

Reviewers check:

- conformance to ADR 0048 idiom rules;
- absence of sealed-corpus access;
- passing `corpus-run-tacit` once that Stage 4 harness exists;
- passing sidecar effect checks;
- no per-task primitive additions unless a follow-up ADR amends
  [ADR 0047](0047-p3-stdlib-expansion-surface.md);
- no algorithmic or layout choices made only to manipulate token counts.

## Alternatives considered

- **Keep human-only Tacit references.** Rejected. It preserves independence
  from model quirks but blocks progress when project contributors do not
  know Tacit well enough to author 47 references unaided. It also conflicts
  with Tacit's AI-first premise.
- **Allow unrestricted LLM assistance without disclosure.** Rejected. The
  reference set would be hard to audit later, and changes in authoring model
  quality could be mistaken for changes in Tacit expressiveness. The
  authorship manifest keeps this visible.
- **Use the same small model being evaluated to author references.**
  Rejected. The reference set should represent expert feasibility, not the
  capability being tested. Stage 9 asks whether smaller-context models can
  solve tasks from the primer; it should not also depend on those same
  models creating the answer set.
- **Give the evaluated model the expert-agent reference as a target to
  replicate.** Rejected. That would test copying or translation, not Tacit
  fluency. The harness continues to grade behavior through compile,
  typecheck, and tests.
- **Move Tacit references out of the corpus tree.** Rejected. ADR 0048's
  per-task layout is still the simplest shape for `corpus-run-tacit`,
  token counting, and review.

## Consequences

- **Stage 4 can proceed with Codex-authored references.** The remaining
  arithmetic/string cleanup and missing string references are no longer
  blocked by a human-only authorship rule.
- **ADR 0048 remains binding except for authorship.** Style, file layout,
  effect annotation, primitive use, and test conformance rules still apply.
- **The benchmark remains uncontaminated if ADR 0049 and ADR 0052 are
  enforced.** The evaluated model does not see references; the primer does
  not paste them; sealed tasks remain unread.
- **The Tacit token comparison remains meaningful.** Tacit references are
  now expert-agent baselines. They measure what the language can express
  under full ADR context, while Stage 9 measures whether primer-only models
  can reach the same behavioral competence.
- **Future papers/results must disclose this authorship mode.** Any Phase 3
  report should state that `reference.tac` files were expert-agent authored
  under ADR 0057, not unaided-human authored.

## Related decisions

- [ADR 0019](0019-corpus-idiom-rules.md) — Python/Rust reference idiom
  rules; this ADR changes only Tacit reference authorship.
- [ADR 0020](0020-sealing-held-out-in-repo.md) — sealed-corpus boundary.
- [ADR 0047](0047-p3-stdlib-expansion-surface.md) — primitive surface;
  new primitive needs still require follow-up ADRs.
- [ADR 0048](0048-p3-tacit-idiom-rules.md) — amended authorship rule.
- [ADR 0049](0049-p3-examples-layout-contamination.md) — primer/reference
  contamination boundary, unchanged.
- [ADR 0051](0051-p3-tacit-token-rule.md) — Tacit authoring-view token
  counting, unchanged.
- [ADR 0052](0052-p3-eval-model-contract.md) — prompt boundary, unchanged.
- [ADR 0056](0056-p3-stage-1-frozen.md) — requires post-freeze amendments
  to be made by new ADR.
