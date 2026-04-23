# 0017 — Stage 3 view-system spec frozen

**Status:** Accepted
**Date:** 2026-04-22
**Phase:** 0, Stage 3 (exit)
**Supersedes:** None
**Spec docs frozen by this ADR:**
- [plans/sidecar-format.md](../plans/sidecar-format.md)
- [plans/inspection-view.md](../plans/inspection-view.md)
- [plans/candidates/authoring-bpe-compact.md § Projection rules](../plans/candidates/authoring-bpe-compact.md)

## Context

Stage 3 of Phase 0 ([phase-0-plan.md § Stage 3](../plans/phase-0-plan.md)) produces the view-system spec that sits on top of the canonical text format frozen by [ADR 0013](0013-canonical-text-format-frozen.md). Its deliverables:

1. Sidecar format for display metadata (Q5) — [ADR 0014](0014-sidecar-format.md).
2. Inspection view scope + surface form — [ADR 0015](0015-inspection-view-scope.md).
3. Rust AST enum location — [ADR 0016](0016-rust-ast-enum-location.md).
4. Authoring-view projection rules (authoring ↔ canonical) — appended to [candidates/authoring-bpe-compact.md](../plans/candidates/authoring-bpe-compact.md).

The structural decisions (ADRs 0014–0016) were accepted at Stage 3 kickoff. What remained for the exit was validating that the three spec documents are **internally consistent** and that an independent implementer could reproduce the worked examples byte-for-byte — the exit criterion from [ADR 0015 § Consequences](0015-inspection-view-scope.md).

Unlike Stage 2, Stage 3 has no two-implementation byte-equivalence gate, because the inspection-view renderer and the authoring-view round-trip tooling are Phase 1+ work. The freeze is therefore a spec-review gate, not a built-artifact gate.

### Review findings (2026-04-22)

A skeptical read of the three spec docs surfaced four blocking inconsistencies and two non-blocking notation slips. All blocking items were fixed in-place before this ADR was written; the fixes are summarized here so the review trail survives the freeze.

**Blocking fixes applied:**

1. **Sidecar § 8 record canonical form used `@`-prefixed syms.** The example wrote `(record @fst (int 1) @mid (int 3) @snd (int 2))`, but every Stage 2 test vector emits record field-syms bare (e.g. `05-record-case-mixed.canonical` → `(record A (int 4) aa (int 2) ...)`), and both Stage 2 canonicalizers emit bare-sym form. The `@` prefix is authoring/inspection-view decoration only, per [inspection-view.md § 3.11](../plans/inspection-view.md) and [canonical-text-format.md § 3](../plans/canonical-text-format.md). Fixed: the § 8 example now reads `(record fst (int 1) mid (int 3) snd (int 2))`.

2. **Authoring projection `rec` rule was self-contradictory about stack discipline.** The original text said "`X0` is innermost index 0 per [ADR 0007](0007-debruijn-rec-indexing.md), so *all* names are pushed together in order `X0`, `X1`, …, `XN-1` with `X0` deepest" — which simultaneously claimed `X0` was innermost (index 0) and deepest (farthest from stack top). An implementer following "push in order" would end up with `X_{N-1}` at index 0, violating ADR 0007. Fixed: the rule now specifies the contract ("lookup of `(var K)` returns `X_K`") and notes, as implementation guidance, that an innermost-at-head stack achieves this by pushing `X_{N-1}, …, X_0` in reverse.

3. **Sidecar § 3.4 truncation vs § 4 staleness rules conflicted.** § 3.4 allowed trailing `null` entries to be omitted; § 6's "compressed" worked example relied on omitting the `children` key entirely on nodes with ≥ 1 canonical child. § 4 said "the array's length, if present, must equal the AST node's canonical child count — otherwise stale," which would flag every compression as stale. Fixed: § 3.4 now explicitly defines the truncation rule (`K ≤ N`, missing key ≡ `K = 0`, implicit `null`s extend to length `N`); § 4 now treats `K ≤ N` as compatible and only `K > N` or kind/key contradictions as stale.

4. **Inspection view § 6 fixtures did not fall out of § 3 rules.** All three "Stage 3 exit fixtures" (§ 6.1 identity-of-5, § 6.2 mutual recursion, § 6.3 hole) used break/inline forms that contradicted § 3's stated "trivial ≤ 40 columns" rule. For example § 6.1's L0 broke `B` to a new line despite both `V` and `B` being trivial under the stated definition. Fixed by reworking § 3 rather than the fixtures, because the fixtures reflect the readability intent of ADR 0015 (always-break compound kinds make structure visible) while the old rules reflected an underspecified width heuristic. The new § 3:

   - Defines **inline** as "a subtree whose L0 rendering contains no line breaks," determined recursively from the per-kind rules (no column-width threshold at L0).
   - Declares `let`, `rec`, `module`, `if`, `match` as **always-break kinds** (never inline at L0).
   - Makes `lam` / `arm` inline iff their body is inline; `app` / `ctor` / `pat-ctor` / `record` (single-field) / `proj` / `ann` inline iff all their children are inline; `hole` always inline.
   - All three § 6 fixtures now reproduce deterministically from the § 3 rules. The L0+L2 and L1+L2 combined fixtures in § 4.3 and § 6.1 were updated to reflect the new inline-let shape.

**Non-blocking fixes applied:**

5. Sidecar § 5 pat-var synthetic-naming wording clarified: `p0`, `p1`, … track *textual* order; canonical DeBruijn order is the *reverse* of that (first textual pat-var gets the highest DeBruijn index). The original parenthetical "same order canonical uses to assign DeBruijn indices" was ambiguous.

The one remaining notation slip (§ 9 writes "`(pat-var)` is nullary canonically" where canonical actually emits bare `pat-var` per the Stage 2 tag-emission convention) was left alone — it's plainly a descriptive reference to the node kind, not a claim about canonical bytes, and fixing it cleanly requires the Stage 2 spec to state the bare-tag-for-nullary emission rule explicitly. Out of scope for Stage 3.

## Decision

**Stage 3 is frozen.** The three view-system spec documents —
[plans/sidecar-format.md](../plans/sidecar-format.md),
[plans/inspection-view.md](../plans/inspection-view.md), and
[plans/candidates/authoring-bpe-compact.md § Projection rules](../plans/candidates/authoring-bpe-compact.md)
— together with the structural decisions in ADRs 0014, 0015, and 0016, are the Stage 3 deliverables, and the exit criterion from [ADR 0015 § Consequences](0015-inspection-view-scope.md) ("an independent implementer could reproduce the worked examples byte-for-byte") is met.

Concretely:

1. **Sidecar format is locked.** The top-level schema (`tacd_version`, `targets_hash_blake3`, `display`), the metadata keys (`binder`, `binders`, `field_order`, `comment`), the `children`-truncation rule, the staleness detection rule, and the synthetic-name scheme are frozen. Reserved-for-Phase-1+ keys (`effect_hint`, `source_range`, `type_hint`, `diagnostic_extra`) are reserved; Phase 0 sidecars must not emit them.

2. **Inspection view L0 rules are locked.** The kind table in § 3 (including the always-break kinds and the "inline iff children inline" rules), the L1 DeBruijn overlay in § 4.1, the L2 hash-badge overlay in § 4.2, and the § 6 worked-example fixtures are regression anchors. Changes that alter L0 output on the § 6 fixtures require a new ADR superseding or amending this one.

3. **Authoring ↔ canonical projection is locked.** The per-construct Direction-1 and Direction-2 rules, the round-trip guarantees (identity for fresh sidecars; synthetic fallback for stale/missing), and the hole-lossiness acknowledgment are frozen.

4. **The Rust AST enum at [impls/rs-canonicalizer/src/ast.rs](../impls/rs-canonicalizer/src/ast.rs) is the Stage 3 conforming transcription** per [ADR 0016](0016-rust-ast-enum-location.md). Promotion to a shared workspace crate is deferred to Phase 1.

5. **Changes to any of these artifacts after this freeze require a new ADR** per [CLAUDE.md § Ground rules](../CLAUDE.md), identical to the Stage 2 freeze discipline imposed by [ADR 0013](0013-canonical-text-format-frozen.md).

## Alternatives considered

- **Defer freeze pending a Phase 1 renderer.** Would let the actual renderer surface specification gaps before we commit. Rejected: Phase 0 is explicitly spec-only ([CLAUDE.md § Ground rules](../CLAUDE.md) — "Don't write a parser, AST walker, or LLVM emitter until Phase 0's exit criteria are met"). Deferring freeze until Phase 1 exists would either violate the ground rule or leave Phase 0 open indefinitely. Spec-level review with the § 6 fixtures as reproducibility anchor is the substitute for a built-artifact gate.

- **Tighten Stage 3 with a two-renderer byte-equivalence gate.** Symmetric to Stage 2's two-canonicalizer gate. Rejected: would require writing two renderers (compiler-adjacent code) in Phase 0, again violating the "no compiler scaffolding" rule, and would exercise Phase 1+ concerns that don't benefit from early pinning.

- **Keep the old "trivial ≤ 40 cols" width-based rule and rewrite the fixtures to match.** Would preserve the original rule and force the fixtures to render the let/if fully-inline when width allowed. Rejected: the original fixtures captured the readability intent of the inspection view (ADR 0015's "structure should be legible at a glance"), and a reader scanning for control flow benefits from `let`/`if`/`match`/`rec` always breaking regardless of width. The structural always-break rule is simpler to implement deterministically and matches what mature code reviewers already expect from pseudo-code.

- **Add L2 hash-badges to leaf nodes too.** Would close the tiny observational gap where someone debugging content-addressing wants a leaf's hash. Rejected: leaves' canonical forms are already short enough that a reader can compute the hash mentally or with a one-liner; badges on every `var`/`int`/`sym` would quadruple the noise for the common debugging case. Revisit if Phase 4 `tacit-debug` demand shows otherwise.

## Consequences

- **Phase 1 may begin view-system implementation against a frozen spec.** The renderer (for inspection view) and the canonicalizer's authoring-view parser can target the frozen rules directly; spec ambiguity discovered during implementation is a Phase-0 bug per [CLAUDE.md § Ground rules](../CLAUDE.md), not scope creep.

- **L0 output on § 6 fixtures is a regression anchor.** Any future change that alters those renderings requires a new ADR. L1 and L2 overlays have a softer stability promise per [ADR 0015](0015-inspection-view-scope.md) but are still specified as worked examples at this freeze.

- **Sidecar schema is additively extensible.** Phase 1+ can add new metadata keys (e.g., `type_hint` when local inference lands in Phase 2) without a version bump, as long as readers ignore unknown keys. Shape changes (structural reorganization of the parallel tree, or introduction of a content-hash overlay layer) would bump `tacd_version`.

- **Phase 0 is one ADR from exit.** After this freeze, the remaining Phase 0 work is Stage 4 (evaluation corpus; 50–100 tasks, ~20% sealed held-out) and Stage 5 (repo scaffolding: Cargo workspace, CI). Neither gates on canonical-form or view-system spec work; both can proceed in parallel.

- **Two of the Stage 2 "open items" in [canonical-text-format.md § 11](../plans/canonical-text-format.md) remain open** and are carried into Stage 4: the `ann` type-subset and the bpe-compact corpus-shape recheck. This ADR does not touch them.

- **The authoring-view projection rules are now the authoritative reference for any canonicalizer that also accepts authoring-view input.** Today's canonicalizers (py / rs) only accept canonical text; a future bidirectional canonicalizer must conform to the § Projection rules doc directly.

## Related decisions

- [ADR 0013](0013-canonical-text-format-frozen.md) — Stage 2 canonical-form freeze; the foundation this stage projects from.
- [ADR 0014](0014-sidecar-format.md) — sidecar parallel-tree format; frozen here.
- [ADR 0015](0015-inspection-view-scope.md) — inspection-view scope; frozen here.
- [ADR 0016](0016-rust-ast-enum-location.md) — Rust AST enum placement; closed here.
- [ADR 0003](0003-authoring-view-bpe-compact.md) — authoring-view grammar, now extended with formal projection rules to canonical + sidecar.
- [ADR 0007](0007-debruijn-rec-indexing.md) — rec/module DeBruijn convention; the authoring-projection `rec` rule was clarified to match this.
- [phase-0-plan.md § Stage 3](../plans/phase-0-plan.md) — the deliverable list this ADR closes. Stage 3 status updates to **Frozen** concurrently with this ADR.
