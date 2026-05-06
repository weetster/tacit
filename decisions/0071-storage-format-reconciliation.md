# 0071 — Storage format reconciliation: `.tac` is canonical, `.taca` is transient authoring

**Status:** Accepted
**Date:** 2026-05-06
**Phase:** Outside the phase system (spec-bug repair).
**Plan:** [plans/canonical-storage-reconciliation.md](../plans/canonical-storage-reconciliation.md)
**Affirms:** [ADR 0013](0013-canonical-text-format-frozen.md), [ADR 0014](0014-sidecar-format.md)
**Supersedes (format only, not test conventions):** [ADR 0043](0043-p2-test-conventions.md) for the `.tac.sidecar.toml` file format

## Context

Phase 0 specified a clean separation between the canonical text format (the byte-exact, content-addressed AST projection) and the authoring view (the BPE-friendly surface AI reads and writes). [Canonical-text-format.md § 0](../plans/canonical-text-format.md) is unambiguous: "It is **not** what humans or AI write. The authoring view ([ADR 0003](0003-authoring-view-bpe-compact.md)) handles writing; the inspection view (Stage 3) handles reading." [Tacit-plan.md § Storage format](../plans/tacit-plan.md) pins the file extension as `.tac` for canonical text with `.tacd` for the JSON sidecar.

[ADR 0013](0013-canonical-text-format-frozen.md) and [ADR 0014](0014-sidecar-format.md) froze that separation as the storage substrate of the project.

Phase 1's implementation drifted. By the time Phase 1 froze, the working pipeline read `.tac` files containing **authoring-view text**, parsed directly by the compiler. The sidecar shipped as `.tac.sidecar.toml` (TOML, types/effects only — no names, no comments, no field order), per [ADR 0043](0043-p2-test-conventions.md)'s test conventions. The canonical form exists only as an in-memory AST representation used for hashing and round-trip tests; nothing canonical is persisted at rest.

The phase-1, phase-2, and phase-3 freeze ADRs ([0033](0033-phase-1-frozen.md), [0046](0046-p2-stage-5-frozen.md), [0070](0070-p3-frozen.md)) reference "`.tac` files written in the authoring view" in passing. That wording is descriptive of the implementation as shipped, not a normative claim about the storage format.

### What the drift retires

The Phase 0 design depends on canonical-as-stored for several load-bearing properties:

- **Rename-free hashes.** Canonical form has no names; renaming a binder is a sidecar-only edit and does not move the content address. Authoring-as-storage turns every rename into a body diff with a hash change.
- **Structural subtree dedup.** Identical AST bodies in different scopes share content addresses automatically because canonical form uses DeBruijn. Authoring-as-storage breaks this — `f x` and `g y` produce different bytes for the same AST shape.
- **Two-implementation byte agreement.** [ADR 0013](0013-canonical-text-format-frozen.md) commits two implementations to identical bytes for the same AST. Authoring text encodes user-chosen names, so the property is meaningless under authoring-as-storage.
- **Structural diff/merge/blame.** [tacit-plan.md § Version control](../plans/tacit-plan.md) plans Git drivers that distinguish body changes from metadata changes. The distinction only exists mechanically when storage separates canonical bytes from sidecar metadata.
- **Content-addressed object store.** [tacit-plan.md § Object store](../plans/tacit-plan.md) requires hash-keyed dedup of definitions across projects. Authoring-as-storage breaks coherence: the same definition under two aliases hashes differently.

Phase 4+ work (cross-project imports, AST-edit protocol stretch target from `tacit-plan.md:99`, structural blame) leans on these properties. Reconciling now is cheaper than threading workarounds through every later phase.

## Decision

**Three file types, well-separated roles:**

| Extension | Contents | Hashed | Checked in | Notes |
|---|---|---|---|---|
| `.tac`    | Canonical text per [canonical-text-format.md](../plans/canonical-text-format.md)              | Yes | Yes                       | Source of truth |
| `.tacd`   | JSON sidecar per [sidecar-format.md](../plans/sidecar-format.md)                              | No  | Yes                       | Names, comments, field order, types, effects |
| `.taca`   | Authoring view per [authoring-bpe-compact.md](../plans/candidates/authoring-bpe-compact.md)   | No  | Only as historical record | Transient render; not produced by the standard dev workflow |

**Regular development does not produce `.taca` files on disk.** The compile workflow reads `.tac` + `.tacd`. Tooling renders authoring view on demand for human or AI consumption; LLMs emit authoring view as strings to the canonicalizer, not files to disk. The `.taca` extension exists for the cases where authoring view *must* touch the filesystem — CLI piping, transient round-trip artifacts, and (the deliberate exception below) preservation of pre-reconciliation research artifacts.

**Phase 3 research artifacts are preserved as `.taca`.** The 47 open `reference.tac` solutions, the 12 round-2 `reference.stdlib.tac` solutions, and the three Phase 3 carry-over programs ([ADR 0070](0070-p3-frozen.md)) were the falsification surface for Phase 3. The exact authoring-view bytes the LLMs were evaluated against are part of the research record. They are preserved as `.taca` (with their paired `.taca.sidecar.toml` TOML sidecars) alongside the newly-emitted canonical `.tac` + `.tacd`. This is the only directory class where `.taca` files are checked in, and is justified because Phase 3 was the project's design and experimentation phase — the bytes are evidence, not source.

**No global `*.taca` gitignore.** The default "not checked in" property comes from the workflow not producing these files in the first place, not from an ignore rule. A repository-wide ignore would block the Phase 3 preservation case without buying anything else, since spurious `.taca` files don't appear under normal use.

Concretely:

1. **`.tac` reverts to its ADR 0013 meaning.** Canonical text only. Compile, check, view, and hash all operate on canonical bytes.
2. **`.tacd` is the only sidecar format.** JSON parallel-tree per [ADR 0014](0014-sidecar-format.md) and [sidecar-format.md](../plans/sidecar-format.md). The type/effect blocks introduced for `.tac.sidecar.toml` ([ADR 0043](0043-p2-test-conventions.md)) are folded into the `.tacd` schema as additive keys; the `.tac.sidecar.toml` file format is retired.
3. **`.taca` is the on-disk extension for authoring view** when it must touch the filesystem (CLI piping, IDE scratch buffers, transient round-trip artifacts). It is gitignored by default. Authoring view as a *render* (in-memory string, terminal output, IDE buffer) does not require an extension at all.
4. **Tooling provides explicit canonicalize and render steps.** `tacit canonicalize <foo.taca>` writes `foo.tac` + `foo.tacd`. `tacit render <foo.tac>` produces authoring or inspection view to stdout or `.taca`. `tacit compile` and `tacit check` accept `.tac` directly; for convenience they may accept `.taca` and canonicalize internally without persisting.
5. **Existing artifacts migrate in two modes.** Phase 3 research artifacts (`corpus/<task>/reference.tac`, `corpus/<task>/reference.stdlib.tac`, `examples/phase-3/*.tac`) are renamed to `.taca` (with paired sidecars renamed to `.taca.sidecar.toml`) and accompanied by newly-emitted canonical `.tac` + `.tacd` — the historical bytes are preserved. Test fixtures (`examples/smoke/*.tac`) are converted in place; original authoring text is not preserved. Both modes are hash-stable per the migration plan's dry-run gate. See [plans/canonical-storage-reconciliation.md § Repository conversion](../plans/canonical-storage-reconciliation.md) for the procedure.

## What stays frozen, what changes

- **Frozen artifacts stay frozen.** ADRs [0013](0013-canonical-text-format-frozen.md), [0014](0014-sidecar-format.md), [0033](0033-phase-1-frozen.md), [0046](0046-p2-stage-5-frozen.md), [0070](0070-p3-frozen.md) and the artifacts they cover (canonical grammar, sidecar JSON shape, smoke corpus deliverables, type/effect coverage, primer + corpus references) remain frozen. The reconciliation reformats data; it does not change semantics.
- **Wording in freeze ADRs.** The phrase "`.tac` files written in the authoring view" in ADRs 0033, 0046, 0048, 0049, 0057, 0070 is historical commentary on the implementation as shipped. No edits to those ADRs.
- **`.tac.sidecar.toml` retires.** The TOML format introduced by ADR 0043 is superseded for storage. The *test conventions* of ADR 0043 (where annotations live, what `[types]`/`[effects]` blocks contain) survive, just expressed in JSON inside `.tacd`.
- **Authoring grammar unchanged.** [ADR 0003](0003-authoring-view-bpe-compact.md)'s bpe-compact grammar is unaffected; only its on-disk role changes (now `.taca` if persisted, more often a transient render).
- **Inspection view unchanged.** Already display-only and round-trip-exempt per [ADR 0015](0015-inspection-view-scope.md).

## Alternatives considered

- **Reconcile spec to reality.** Amend [tacit-plan.md § Storage format](../plans/tacit-plan.md), [ADR 0013](0013-canonical-text-format-frozen.md), and [ADR 0014](0014-sidecar-format.md) to declare `.tac` = authoring view, canonical form = in-memory hashing projection, sidecar = TOML. **Rejected.** This would quietly retire the rename-free-hash, structural-dedup, two-implementation-agreement, and structural-diff properties the design depends on. The spec is the load-bearing artifact; the implementation is what catches up. Per CLAUDE.md ground rules, "spec ambiguities are bugs against the relevant frozen artifact" — this is the ground rule operating as designed.
- **Status quo + a clarifying ADR.** Document that `.tac` denotes authoring view in practice, accept the lost capabilities, defer reconciliation indefinitely. **Rejected.** Phase 4 work (cross-project imports, structural-edit infrastructure, blame) would compound the drift. Easier to pay the migration cost now than to plumb workarounds through every later phase.
- **Single extension, format-sniffed.** Keep `.tac` as the only extension and detect canonical vs. authoring by content. **Rejected.** Format ambiguity at the file level is a footgun (every tool needs the sniffer; mixed-state directories silently misbehave). Two extensions cost nothing and are unambiguous.
- **Different name than `.taca`.** `.tact`, `.tacw` (writing), `.tacin` were considered. `.taca` (authoring) reads cleanly, fits the existing `.tac` / `.tacd` family, and is what was proposed when the question was raised.
- **Persist `.taca` alongside `.tac` everywhere.** Treat authoring view as a co-equal stored artifact for every program. **Rejected.** Authoring view is determined by canonical AST + sidecar; storing it duplicates information and creates a third source of truth that can drift. The render-on-demand model is the natural default. The Phase 3 research-artifact carve-out is a bounded historical-record case, not a recommendation to generalize.
- **Globally gitignore `*.taca`.** Block all checkins by ignore rule. **Rejected.** Would block the Phase 3 preservation case without buying enforcement (the workflow doesn't produce `.taca` files at the relevant scale anyway). Convention plus reviewer attention is sufficient.

## Consequences

- **Renames cost zero hash churn.** Once canonical-as-stored lands, renaming a binder produces a `.tacd`-only diff. Body bytes don't move; content addresses don't move; structural blame stays clean.
- **Subtree dedup in the in-memory AST becomes correct by construction.** Identical bodies in different scopes share hashes; the deferred object store ([tacit-plan.md § Object store](../plans/tacit-plan.md)) becomes implementable when imports arrive.
- **Two-implementation byte agreement becomes a real test target.** A second implementation can be validated against canonical bytes, not authoring text whose names depend on author choice.
- **Phase 3 corpus eval gains hash equality as an equivalence check.** LLM output canonicalizes to a hash; comparison against the reference's canonical hash is stronger than text equality.
- **Migration is mechanical but not zero-cost.** ~60 reference solutions, ~30 smoke programs, ~3 carry-over programs, and round-trip tests all migrate. The plan ([plans/canonical-storage-reconciliation.md](../plans/canonical-storage-reconciliation.md)) requires a hash-stability dry-run before commit.
- **Phase 4 dependency.** Structural-edit and language-shape work built atop authoring-as-storage would need rework. Either reconcile first, or accept Phase 4 ships alongside this and structural-edit work waits for completion. The plan recommends reconciliation first.
- **No global `*.taca` ignore.** The default "not checked in" property comes from workflow shape, not enforcement. The Phase 3 research-artifact preservation case is the explicit checked-in exception.
- **Documentation passes follow.** [CLAUDE.md](../CLAUDE.md), [tacit-plan.md](../plans/tacit-plan.md), [canonical-text-format.md](../plans/canonical-text-format.md), [sidecar-format.md](../plans/sidecar-format.md) get cleanup edits to reference the three-extension model directly. Primer wording shifts from "the contents of a `.tac` file" to "authoring view" where the distinction matters.

## Related decisions

- [ADR 0013](0013-canonical-text-format-frozen.md) — canonical text format frozen. Affirmed; this ADR is implementation catching up.
- [ADR 0014](0014-sidecar-format.md) — `.tacd` JSON sidecar. Affirmed; extended with type/effect blocks from ADR 0043.
- [ADR 0003](0003-authoring-view-bpe-compact.md) — authoring view grammar. Unaffected; only its on-disk role changes.
- [ADR 0015](0015-inspection-view-scope.md) — inspection view. Unaffected.
- [ADR 0033](0033-phase-1-frozen.md) — Phase 1 freeze. Wording about "`.tac` files written in the authoring view" becomes historical commentary; deliverables stay frozen.
- [ADR 0043](0043-p2-test-conventions.md) — `.tac.sidecar.toml` file format superseded; test conventions (where annotations live) preserved inside `.tacd`.
- [ADR 0046](0046-p2-stage-5-frozen.md), [ADR 0070](0070-p3-frozen.md) — phase freezes; deliverables stay frozen, file format changes mechanically.
- [plans/canonical-storage-reconciliation.md](../plans/canonical-storage-reconciliation.md) — implementation plan for this ADR.
