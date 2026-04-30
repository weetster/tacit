# 0048 — Tacit-Lite reference-solution idiom rules

**Status:** Accepted
**Date:** 2026-04-28
**Phase:** 3, Stage 1
**Closes:** [phase-3-plan.md Q-P3-2](../plans/phase-3-plan.md)
**Amended by:** [ADR 0057](0057-p3-expert-agent-tacit-reference-authorship.md)

## Context

[ADR 0019](0019-corpus-idiom-rules.md) pins reference-solution idiom for
Python and Rust because "equivalent Python" is underspecified — the same
task can vary 2–3× in token count between idiom choices, and the 30%
Phase 3 reduction gate is smaller than that variance. The Tacit-Lite
side of the comparison has the same problem in a sharper form.

Tacit-Lite admits stylistic latitude that the language itself does
nothing to constrain:

- `let f = lambda x. e` versus `lambda x. e` at the call site.
- `rec { f = … }` for a self-recursive function versus a non-`rec`
  fixed-point encoding.
- `@scan-byte` followed by `@buf-eq` versus a hand-rolled equality loop.
- Generic `forall a. List a → i64` versus monomorphic `List i64 → i64`
  for a function that is called only on `i64`.
- Compact authoring-view text versus a layout that mirrors Python
  paragraph structure.

The same authors write Python references (under ADR 0019), Tacit-Lite
references (this ADR), and the primer (under
[ADR 0050](0050-p3-primer-scope.md)). Without a written rule, those
authors have a knob that reaches further than the 30% reduction gate
— exactly the failure mode ADR 0019 § Decision was written to prevent
on the Python side.

The Q-P3-2 deferral in [phase-3-plan.md](../plans/phase-3-plan.md) also
asked whether references ship as `reference.tac` plus a sidecar in each
task directory, or as a single combined artifact. The smoke-corpus
precedent ([`examples/smoke/`](../examples/smoke/)) settles this: each
program ships as `<name>.tac` plus `<name>.tac.sidecar.toml`. Phase 3
references follow the same pattern.

## Decision

**Tacit-Lite reference solutions are pinned to a single idiomatic
style. Each reference ships as `reference.tac` plus
`reference.tac.sidecar.toml` in its task directory. The Python
reference under ADR 0019 remains the sole Phase 3 token-count
baseline; Tacit-Lite token counts under
[ADR 0051](0051-p3-tacit-token-rule.md) are the comparison side.**

### File layout

For each open corpus task `<category>/<NNN-slug>/`:

- `reference.tac` — authoring-view source.
- `reference.tac.sidecar.toml` — sidecar carrying `[types.<binding>]`
  blocks per [ADR 0043](0043-p2-test-conventions.md), and any
  display-name metadata required by the round-trip property of
  [ADR 0033 § 2](0033-phase-1-frozen.md).

Sealed tasks (002, 008, 014, 019, 022, 024, 029, 034, 039, 043, 048,
053, 058) get **no** Tacit-Lite reference; the model authors those at
eval time and the Python reference is the only baseline. This is a
natural consequence of [ADR 0020](0020-sealing-held-out-in-repo.md):
Tacit material in `sealed/` would have to be sealed-hashed and would
provide no eval signal anyway.

### Authoring view

- **Format:** authoring view per
  [`plans/canonical-text-format.md`](../plans/canonical-text-format.md)
  and the Phase 2 amendments in ADRs 0034–0039. Round-trips against
  the canonical parser ([ADR 0033 § 2](0033-phase-1-frozen.md)).
- **No comments.** The task statement is the documentation. Adding
  authoring-view comments inflates the Tacit-Lite token count without
  representing what the model under test would produce. Mirrors the
  Python "no docstrings" rule from ADR 0019.
- **No display-name renaming for stylistic effect.** Display names in
  the sidecar reflect what an author would type; renaming an
  argument from `n` to `count` for legibility is fine, renaming for
  token-budget gaming is not. If two display names work equally well,
  the shorter wins (mirror of ADR 0019's "shorter variant wins" rule).
- **Whitespace:** one expression per line for top-level `let`, inline
  for nested expressions where they fit. No multi-line layout for
  tokenizer manipulation.

### Recursion and binding

- **Use `rec { … }` for self-recursion or mutual recursion.** A
  function that calls itself goes inside `rec`; a function that does
  not, does not. `rec` introduces overhead in canonical form; using
  it as a uniform wrapper inflates token counts.
- **Top-level definitions are named.** A function that is the
  task's main computation is bound by `let name = lambda … in …`,
  not inlined into `main`. Inlining a 4-line function into `main`
  shaves tokens at the cost of legibility; the rule favours
  legibility because the model under test will write named functions.
- **Single-call-site lambdas are inline.** A lambda used in exactly
  one call site is written inline (`lambda x. e`), not bound and
  named. Naming a single-use lambda costs tokens for no benefit.

### Primitive use

- **Use the corpus primitive when one fits.** ADR 0047 introduces
  PARSE, FORMAT, and MEM primitives precisely so that references
  do not hand-roll integer parsing or byte equality. A reference
  that hand-rolls `@parse-i64` from `@buf-get` and ARITH ops is
  rejected at PR review.
- **Mirror Python stdlib choices.** Where ADR 0019 pins Python to use
  `bisect`, `Counter`, `dict.fromkeys`, etc., the Tacit reference
  uses the most direct primitive composition that achieves the same
  effect. Hand-rolling for token-count advantage is rejected;
  hand-rolling because no primitive fits is acceptable and triggers
  a Q-P3-1 follow-up per ADR 0047 § Consequences.
- **No primitive aliasing.** `let plus = lambda x y. @add x y` to give
  `@add` a friendlier name is forbidden. The primitive is the call.

### Generics

- **Monomorphic by default.** A reference that is called only on
  `i64` is typed at `i64`. `forall a. List a → i64` for a length
  function is forbidden where `List i64 → i64` suffices for the
  task. Generics in references are only justified when the same
  function is reused across types within the same reference, and
  even then only at module-export boundaries per
  [ADR 0034 § 2](0034-p2-type-subset-ann.md).
- **No effect polymorphism in references.** All references are
  monomorphic at concrete effect sets. Effect polymorphism is a
  language feature exercised by the smoke corpus and any
  primer-driven examples; corpus references are end-to-end programs
  with concrete effects (`{IO}` for the `main` boundary, `{}` or
  `{Mut}` internally) and ADR 0036's effect-variable surface is
  unused here.

### Effect annotations

- **Mandatory at module boundaries.** Per
  [ADR 0034](0034-p2-type-subset-ann.md) and
  [ADR 0036](0036-p2-effect-polymorphism-syntax.md), every exported
  binding (i.e., `main`) carries an explicit type-and-effect
  annotation. The reference's `main` is annotated `i64 ! {IO}` or
  the equivalent `fn-ty` form per the Phase 2 surface.
- **Internal bindings are inferred.** Per
  [ADR 0034 § 4](0034-p2-type-subset-ann.md), local `let` bindings
  use Phase 2 inference. References do not over-annotate.

### Algorithm choice

- **Pick the variant an experienced author would reach for first.**
  Same rule as ADR 0019 § Review process. Recursion or iteration
  follows clarity, not token count.
- **Tie-break by token count when both variants are clear.** If two
  algorithms are both reasonable and their Tacit-Lite token counts
  differ by more than 10%, the shorter is chosen — so Python is
  never compared against a strawman Tacit (mirror of ADR 0019).
- **No algorithmic deviation from the Python reference's general
  approach** unless the Tacit version is materially clearer. The
  Python reference is the canonical *shape* of the solution; the
  Tacit version expresses the same shape in Tacit. Replacing a
  linear scan with a binary search for token-count gain (when the
  Python reference uses linear scan) is rejected — that is gaming
  the per-task delta rather than measuring language density.

### Test conformance

Every reference compiles end-to-end via `tacit compile`, typechecks
with the annotated effect signature, and passes every test case in
`tests.jsonl`. The new harness command `corpus-run-tacit`
(introduced in Stage 4 per
[phase-3-plan.md § Stage 4](../plans/phase-3-plan.md)) exercises this
per push. References that fail any test case are not merged.

### Authorship

LLM assistance in writing Tacit-Lite references is **not** permitted.
ADR 0019 permits LLM authorship for Python/Rust because the
comparison target is LLM-generated code in human-language idiom — the
LLM bias is part of what the baseline measures. The Tacit comparison
side is the opposite: it is the *upper bound* on what a careful
human author can express in Tacit-Lite, and LLM assistance would
let the model under test (transitively, via its training data on
LLM-authored Tacit) leak into the baseline. References are
hand-authored.

### Review process

- Each reference is reviewed against these rules at PR time, before
  Stage 6 freeze.
- Disputes about a single reference are resolved by picking the
  variant most consistent with the other Stage 4–6 references. The
  rule is internal consistency, not a per-task aesthetic call.
- Recurring disputes produce a follow-up ADR amending this one, not
  a re-litigation of an existing reference.
- These rules freeze with Stage 6 exit. After freeze, changes
  require a new ADR — same discipline as ADR 0019.

## Alternatives considered

- **No style rule ("use your judgment").** Rejected for the same
  reason as ADR 0019: the idiom variance is larger than the 30%
  gate, so the gate would be measuring author intent rather than
  the language.
- **Allow LLM-assisted Tacit references** (mirror Python rule).
  Rejected. The Phase 3 thesis is that a model can write Tacit-Lite
  *from a primer alone*. If the references the gate is measured
  against are themselves LLM-authored, the comparison degenerates
  into "model with primer + helper LLM vs model with primer
  alone," which is not what the parent plan claims to measure.
- **Single combined artifact per task** (one file containing source
  + sidecar inline). Rejected. The smoke corpus already commits
  to `<name>.tac` + `<name>.tac.sidecar.toml`; mirroring that is
  the cheapest path. A combined artifact would also obscure
  round-trip verification, which operates per-pair.
- **Token-budget annotation in the sidecar** (record the token
  count at PR time as a check against drift). Rejected. The
  harness recomputes token counts from source per push; storing a
  snapshot in the sidecar is duplicative and risks divergence.
- **Allow comments where the Tacit construct has no Python
  analogue** (e.g., explaining `@buf-alloc-dyn` lifetime). Rejected.
  The primer is the place to teach the language; the reference is
  the place to use it. A reference that needs a comment to be
  understandable signals the primer is incomplete, not that the
  reference needs prose.

## Consequences

- **The 30% Phase 3 gate is measurable on the Tacit side.** Combined
  with ADR 0019 and ADR 0021 on the Python side, both halves of the
  comparison are pinned; the gate has no remaining stylistic knobs.
- **Stage 4–6 authoring is mechanical, not creative.** Each task
  gets one solution, judged against the rules in this ADR. Idiom
  disputes become ADRs rather than per-task drift.
- **Primer authoring is constrained.** The primer's progressive
  examples (per [ADR 0050](0050-p3-primer-scope.md)) draw from the
  Stage 4–6 references and inherit their idiom. The model sees one
  flavour of Tacit-Lite, mirroring the Python flavour.
- **Some tasks will feel under-expressive.** A task that benefits
  from a hash map or higher-kinded type cannot use one in its
  reference; the Tacit version will be longer than the Python
  version. Accepted — this is exactly the signal the
  stdlib-dominated tag in ADR 0021 captures, and the corpus is
  calibrated on Tacit-Lite's primitive surface, not Tacit-Full.
- **No LLM assistance in references.** Authorship cost is higher
  than Python reference cost; the budget allowance is captured in
  [phase-3-plan.md § Stage 4–6](../plans/phase-3-plan.md) timeline.
- **These rules freeze with Stage 6 exit.** Same discipline as ADR
  0019. Reference text changes after Stage 6 require a new ADR.

## Related decisions

- [ADR 0019](0019-corpus-idiom-rules.md) — Python/Rust idiom rule;
  this ADR is the Tacit-Lite analogue.
- [ADR 0020](0020-sealing-held-out-in-repo.md) — sealing discipline
  that excludes sealed tasks from Tacit reference scope.
- [ADR 0021](0021-corpus-stdlib-dominance-reporting.md) — stdlib-
  dominated reporting; the per-task Tacit reference is a
  data point in each of the three aggregates.
- [ADR 0033 § 2](0033-phase-1-frozen.md) — round-trip property the
  reference + sidecar pair must satisfy.
- [ADR 0034](0034-p2-type-subset-ann.md), [ADR 0036](0036-p2-effect-polymorphism-syntax.md)
  — type/effect annotation rules at module boundaries.
- [ADR 0043](0043-p2-test-conventions.md) — `[types.<binding>]`
  sidecar block format.
- [ADR 0047](0047-p3-stdlib-expansion-surface.md) — primitive surface
  references draw from.
- [ADR 0050](0050-p3-primer-scope.md) — primer scope; the references
  are primer-example sources for the open subset.
- [ADR 0051](0051-p3-tacit-token-rule.md) — Tacit-Lite token-count
  rule; what the references are measured under.
