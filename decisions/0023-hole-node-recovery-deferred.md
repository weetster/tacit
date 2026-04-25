# 0023 — Hole-node parser recovery deferred to Phase 2

**Status:** Accepted
**Date:** 2026-04-24
**Phase:** 1, Stage 1
**Closes:** [phase-1-plan.md Q-P1-2](../plans/phase-1-plan.md)

## Context

[CLAUDE.md § Key design commitments](../CLAUDE.md) commits to typed `Hole`
nodes for malformed subtrees: parser error recovery produces `Hole` AST nodes
carrying structured diagnostics, so the rest of the file stays analyzable.
The existing canonical-text parser at
[`crates/tacit-canonical/src/parse.rs`](../crates/tacit-canonical/src/parse.rs)
does not implement this — it hard-fails with a `ParseError` on the first
malformed construct.

The AST enum at [`crates/tacit-canonical/src/ast.rs`](../crates/tacit-canonical/src/ast.rs)
(per [ADR 0016](0016-rust-ast-enum-location.md), promoted to its current
workspace location by [ADR 0029](0029-cargo-workspace-layout.md)) already carries a `Hole`
variant transcribed from the spec, and the canonical emitter and BLAKE3
hasher both handle `Hole` nodes. Canonical text that *contains* an
explicitly-constructed `Hole` parses, hashes, and round-trips today.
The missing piece is specifically parser *recovery* — turning a malformed
subtree into a `Hole` at parse time instead of aborting.

[phase-1-plan.md § Stage 1](../plans/phase-1-plan.md) flags this as Q-P1-2
with the decision framed as: Phase 1 punts (defer to Phase 2) vs. Phase 1
retrofits (Stage 1 scope creep).

Phase 1's only consumer of parse errors is the Stage 5 CLI (`tacit compile`,
`tacit view`), which surfaces the error text and exits non-zero. There is
no editor integration, no incremental compilation, no LSP surface, and no
AI-edit-protocol consumer that would benefit from a partial AST in Phase 1.
Hole-node recovery would need to touch every parse path in the canonicalizer
and every consumer that walks the AST, with no present consumer to validate
the design against.

## Decision

**Phase 1 ships with the existing hard-failing `ParseError` behavior.
Hole-node parser recovery is deferred to Phase 2 or to a dedicated tooling
phase, driven by a concrete consumer.**

Concretely:

1. The `Hole` AST variant stays in the enum unchanged. Canonical text that
   explicitly contains `Hole` nodes continues to parse, hash, and round-trip
   through the views.
2. The parser's error-reporting surface is unchanged from Phase 0. Stage 5's
   CLI surfaces the existing `ParseError` value directly.
3. No Phase 1 work adds `Hole` construction paths to the parser, and no
   Phase 1 downstream consumer (authoring-view parser in Stage 2,
   inspection-view renderer in Stage 3, LLVM emitter in Stage 4) is
   designed around partial ASTs.
4. Phase 2 inherits the work as a known, scoped follow-up: add `Hole`
   recovery to the existing parser, wire it into the Phase 2 structured
   diagnostics surface, and update the CLI to render partial-parse output.

## Alternatives considered

- **Retrofit in Stage 1.** Rejected. Stage 1 is pinned to scaffolding
  (workspace promotion, CI wiring); adding `Hole` recovery is a semantic
  change to the parser that touches every error path and every AST consumer.
  Without a Phase 1 consumer to drive the design, retrofitting guarantees
  we re-design once the real consumer arrives.
- **Ship with `ParseError` permanently.** Rejected. Contradicts a
  load-bearing CLAUDE.md commitment and will be flagged in any external
  review. The deferral is the gap; permanent removal would be a new
  decision that this ADR does not make.
- **Ship a partial recovery (top-level only, no nesting).** Rejected as
  the worst of both options — visible behavior change without proportional
  payoff, and re-design churn when full recovery arrives.
- **Add `Hole` recovery only in the authoring-view parser (Stage 2) while
  leaving the canonical parser strict.** Rejected. The view system per
  [ADR 0015](0015-inspection-view-scope.md) projects from AST to text;
  recovery at the view boundary would silently diverge the two parsers'
  error behavior and complicate round-trip testing.

## Consequences

- Phase 1 ships with a parser that diverges from a frozen design
  commitment. This divergence is logged here and in the phase-1 plan so
  it is intentional, not silent drift.
- Stage 5's CLI error rendering is simpler — one code path, no
  partial-AST awareness.
- The authoring-view parser (Stage 2), inspection-view renderer (Stage 3),
  and LLVM emitter (Stage 4) assume well-formed ASTs; none needs `Hole`
  handling in its walking logic beyond what the AST enum already allows
  for explicitly-constructed `Hole` nodes.
- Phase 2 gets a known, bounded follow-up: Hole recovery is a
  self-contained sub-stage, not a refactor. The AST is already
  recovery-ready; the change is parser-local.
- The AI-edit-protocol stretch target (CLAUDE.md mentions an
  insert/replace/delete protocol against a known-good tree) remains
  an independent design space; neither enabled nor foreclosed by this
  deferral.

## Related decisions

- [CLAUDE.md § Key design commitments](../CLAUDE.md) — the typed-`Hole`
  commitment this ADR temporarily defers.
- [ADR 0013](0013-canonical-text-format-frozen.md) — canonical text format
  freeze; does not mandate recovery-path behavior.
- [ADR 0016](0016-rust-ast-enum-location.md) — AST enum location; the
  `Hole` variant already lives there.
- [phase-1-plan.md § Stage 1, § Open Questions Q-P1-2](../plans/phase-1-plan.md)
  — closed by this ADR.
- Future Phase 2 ADR (number TBD) — will specify the recovery algorithm,
  diagnostics schema carried on `Hole` nodes, and CLI rendering.
