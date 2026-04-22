# 0015 — Inspection view scope: display-only pseudo-code with progressive annotations

**Status:** Accepted
**Date:** 2026-04-22
**Phase:** 0, Stage 3
**Spec:** [plans/inspection-view.md](../plans/inspection-view.md)

## Context

[phase-0-plan.md § Stage 3](../plans/phase-0-plan.md) lists "inspection view grammar + projection rules (indented, type-annotated, effect-annotated)" as a deliverable. [CLAUDE.md](../CLAUDE.md) requires "two views from day one — if only one ships, the view abstraction rots into a single canonical form." This ADR pins the inspection view's scope and design axis; the grammar details live in [plans/inspection-view.md](../plans/inspection-view.md).

The authoring view (per [ADR 0003](0003-authoring-view-bpe-compact.md)) is optimized for token density — it is what an AI writes. The inspection view has the opposite audience and optimization:

- **Audience:** humans doing code review, AI models debugging failing programs, the eventual `tacit-debug` CLI.
- **Optimization:** legibility and information density *per-line*, not total tokens. A reader wants to find structure, types, and anomalies quickly.

### Two framing questions

**Q1: Is the inspection view round-trippable back to canonical bytes?**

[CLAUDE.md](../CLAUDE.md) states the two views are "both lossless projections of the canonical form." That phrase has two plausible readings:

- *Lossless = invertible.* The view is a parseable formal syntax; `canonical → view → canonical` is the identity. Requires a parser, and every bit of visual structure must be grammar.
- *Lossless = no hidden information.* Everything in canonical form is accessible to a reader of the view. The view is display-only; `canonical → view` is a function, not a round-trip.

**Every precedent in debugger tooling takes the second reading.** GDB output, Chrome DevTools DOM inspector, `rustc -Zunpretty=hir`, `clang -ast-dump`, GHCi's `:info` — all are display-only. None have round-trip parsers, and the absence hasn't hurt any of them. The first reading would force the inspection view to trade off visual design for parseability, and the resulting grammar would compete with the authoring view's grammar for no semantic gain (two distinct parseable surface syntaxes is worse than one).

**Q2: What does the inspection view's surface syntax look like?**

Three shape options:

- **Tree-drawing.** Unicode glyphs (`├─`, `└─`, `│`) make structure explicit. Good for visualizing AST shape; bad for reading *semantics* (`├─App` is a structural label, not a legible call site).
- **Tabular/columnar.** Rows are nodes, columns are kind/name/type/etc. Good for machine-greppable output; bad for following control flow, which spans rows.
- **Pseudo-code.** Indented, keyword-heavy, reads like ordinary source. Good for control flow and semantics; weaker at making structural boundaries explicit (what a tree-drawing gives for free).

The pseudo-code form matches what readers are already trained on (every programming language they've read). Tree-drawing and tabular forms can be future optional modes if demand emerges; they are not the default inspection view.

### Constraints from elsewhere in Phase 0

- **Effect annotations are a Phase 1+ feature** (Tacit-Lite's simple effect lattice lands in Phase 2). The inspection view must *reserve a slot* for effect rendering but does not yet render effects in Phase 0.
- **Types are partial.** `ann` nodes carry types; non-annotated nodes don't. Phase 2 adds local inference; Phase 0 renders only what's present in `ann` nodes.
- **Sidecars are advisory** ([ADR 0014](0014-sidecar-format.md)). A missing or stale sidecar must still yield a readable inspection view via synthetic names.
- **Canonical text is 7-bit ASCII** ([ADR 0010](0010-canonical-emission-rules.md)). The inspection view has no such restriction — it's for display, not hashing. Unicode annotations are permitted.

## Decision

**The inspection view is a display-only, pseudo-code-shaped projection of the canonical AST, with progressive annotation density controlled by rendering flags.** The detailed grammar is specified in [plans/inspection-view.md](../plans/inspection-view.md).

Concretely:

1. **Display-only.** Not round-trippable to canonical bytes. Canonical text is the authoritative source of truth; the inspection view is a read-only projection. The "lossless projections" phrase in CLAUDE.md is interpreted as "no hidden information," not "invertible."
2. **Surface syntax: pseudo-code.** Keyword-led, indented, one expression per visual line (with some nodes rendered inline for brevity — rules in [plans/inspection-view.md § 3](../plans/inspection-view.md)). Unicode is permitted for annotations and glyphs; the core identifier and keyword stream is ASCII.
3. **Progressive annotation density.** Three layers, each controlled by a flag:
   - **Default (L0):** names (from sidecar or synthetic), types from `ann` nodes, comments from sidecar, `hole` markers, record fields in authoring order. What a human code reviewer wants.
   - **`--debruijn` (L1):** adds DeBruijn overlays on every `var` reference (`x  /* ≡ var 0 */`).
   - **`--hashes` (L2):** adds per-node BLAKE3 prefix badges (first 8 hex chars).
   - Flags compose (L0+L1+L2 possible). Additional layers (`--types`, `--effects`) are reserved for Phase 1+ when inference or the effect system exist.
4. **Lossless in information content at L0+L1+L2.** Every canonical byte of structure is reachable from the reader via some flag combination. (Phase 1 additions — effects, inference — extend this rather than replacing it.)
5. **Stable output at Phase 0 exit.** The default L0 rendering of the Stage 2 worked examples becomes a Stage 3 regression fixture. Phase 1+ changes that alter those fixtures require an ADR. This is a weaker stability promise than canonical form's byte-equivalence — the inspection view is allowed additive evolution (new annotations, refined whitespace) — but L0 baseline examples are locked against silent drift.
6. **No tree-drawing or tabular output in Phase 0.** Deferred as optional modes if tooling demand emerges. The pseudo-code form is the only Phase 0 inspection-view surface.

## Alternatives considered

- **Round-trippable inspection view (parseable back to canonical).** Rejected. Forces the grammar to compete with the authoring view for parseability without benefit. No debugger precedent requires it. Display-only is the standard for this role.
- **Tree-drawing as the default (`├─`/`└─` Unicode glyphs).** Rejected for the default. Makes *structure* legible but hides *semantics* — readers spend effort reconstructing "this is a function call" from structural labels. Kept as a possible `--tree` mode for the future.
- **Tabular/columnar output (one row per node).** Rejected. Good for grep and machine consumption; poor for following control flow or reading patterns in context. Kept as a possible `--table` mode for machine-parseable use (the debugger's JSON emission in Phase 4 is the natural fit).
- **Single dense form with every annotation on by default.** Rejected. A reviewer scanning for a type error doesn't want per-node hashes; a canonicalizer debugger scanning for a hash mismatch doesn't want full control flow. Progressive disclosure via flags matches the audience's need.
- **Single sparse form with no annotations.** Rejected. Would lose types entirely (reducing the inspection view to "authoring view with whitespace"), and the plan's explicit "type-annotated, effect-annotated" language makes this a non-starter.
- **Effect annotations in the Phase 0 default rendering.** Rejected. No effect system exists until Phase 2; reserving syntactic space without implementing anything is more likely to produce design debt than foresight. The grammar doc reserves the annotation slot; the renderer emits nothing until Phase 2 lands.

## Consequences

- **No parser needed for the inspection view.** Phase 0 produces only a spec and a projection rule. Phase 1+ produces a renderer. There is no Phase-ever reverse parser.
- **Renderer is simpler than a canonicalizer.** It's a one-pass walk over AST + sidecar producing text. No byte-exactness, no stability-under-reordering, no rejection handling — those are canonical-form concerns.
- **Regression fixtures are human-readable.** When the grammar changes, a diff of the rendered examples is the review surface. Compare to canonical form's byte-diffs, which work but need spec-referenced interpretation.
- **L0 fixtures are the stability anchor.** Any change that alters L0 output on the worked examples ([canonical-text-format.md § 10](../plans/canonical-text-format.md)) needs an ADR. L1/L2 fixtures land together with the freeze but are allowed a softer stability promise — they can tighten across phases without retroactive spec ADRs as long as L0 is unchanged.
- **Phase 1+ renderer gets a clear extension point.** New annotation layers (`--types` for inferred types, `--effects` for effect sets) slot in without redesigning the core grammar. Their fixture stability is governed by the phases that introduce them.
- **Inspection-view output may contain Unicode.** Downstream tools (diff viewers, debuggers) must handle UTF-8. Canonical form remains 7-bit ASCII; the constraint does not propagate.
- **The "two views" ground rule is satisfied by a spec + fixtures, not by a shipping renderer.** Stage 3 exit requires [plans/inspection-view.md](../plans/inspection-view.md) with worked examples at L0/L1/L2 that a second implementer could reproduce byte-for-byte. It does not require the renderer itself.

## Related decisions

- [ADR 0003](0003-authoring-view-bpe-compact.md) — authoring view; the density-optimized sibling of this view.
- [ADR 0014](0014-sidecar-format.md) — sidecar format; supplies names, field order, and comments that the inspection view consumes.
- [canonical-text-format.md § 10](../plans/canonical-text-format.md) — worked examples that become this view's L0 regression fixtures.
- [phase-0-plan.md § Stage 3](../plans/phase-0-plan.md) — the deliverable this ADR scopes.
- [tacit-plan.md § Two from day one](../plans/tacit-plan.md) — the "views are load-bearing from day one" framing.
