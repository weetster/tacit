# Canonical Storage Reconciliation

**Status:** Draft
**Scope:** Outside the phase system. Repairs the on-disk storage format to match the Phase 0 specification ([canonical-text-format.md](canonical-text-format.md), [tacit-plan.md § Storage format](tacit-plan.md)). Runs concurrently with or before Phase 4.
**Date:** 2026-05-06

## Motivation

The Phase 0 spec declares that `.tac` files contain **canonical text** — the byte-exact, deterministic AST projection that BLAKE3 hashes ([canonical-text-format.md](canonical-text-format.md)). The authoring view is explicitly *not* what humans or AI write to disk; it is a render layer projected from the canonical AST plus a sidecar of display metadata.

Phase 1 implementation drifted. Today every `.tac` file in the repo contains authoring-view text, parsed directly by the compiler. The sidecar landed as `.tac.sidecar.toml` (TOML) carrying types and effects only — no binder names, no comments, no field order. The canonical form exists only as an in-memory AST; nothing canonical is ever persisted or hashed at rest.

This drift retires capabilities the project's design depends on:

- **Rename-free hashes.** Renaming `n` to `count` should leave canonical bytes and content-addresses untouched. Today it is a full body diff.
- **Structural subtree dedup.** Identical AST bodies in different scopes should share content-addresses automatically (the property that makes the deferred object store coherent — see [tacit-plan.md § Object store](tacit-plan.md)). Today they don't, because authoring text encodes the surrounding names.
- **Two-implementation byte agreement.** [ADR 0013](../decisions/0013-canonical-text-format-frozen.md) commits two implementations to producing identical bytes for the same AST. With authoring-view-as-storage, the property is meaningless because authoring text encodes user-chosen names.
- **Structural diff/merge/blame.** [tacit-plan.md § Version control](tacit-plan.md) plans Git drivers that distinguish body changes from metadata changes. That distinction only mechanically exists if storage separates canonical bytes from sidecar metadata.

Phase 4+ work (cross-project imports, AST-edit protocol stretch target from `tacit-plan.md:99`, structural blame) compounds these problems. Reconciling now is cheaper than plumbing workarounds through every later phase.

## End state

Three file types, well-separated roles:

| Extension | Contents | Hashed | Checked in | Notes |
|---|---|---|---|---|
| `.tac`    | Canonical text per [canonical-text-format.md](canonical-text-format.md)              | Yes | Yes                       | Source of truth |
| `.tacd`   | JSON sidecar per [sidecar-format.md](sidecar-format.md)                              | No  | Yes                       | Names, comments, field order, types, effects |
| `.taca`   | Authoring view per [authoring-bpe-compact.md](candidates/authoring-bpe-compact.md)   | No  | Only as historical record | Transient render; not produced by the standard dev workflow |

**Regular development does not produce `.taca` files on disk.** The compile workflow reads `.tac` + `.tacd`. Tooling renders authoring view on demand for human or AI consumption (terminal output, IDE buffers, LLM context); LLMs emit authoring view as strings to the canonicalizer, not files to disk. The `.taca` extension exists for the cases where authoring view *must* touch the filesystem (CLI piping, transient round-trip artifacts, and — see below — preservation of pre-reconciliation research artifacts).

**Phase 3 corpus and carry-over programs are a deliberate exception.** They were the falsification surface for Phase 3 ([ADR 0070](../decisions/0070-p3-frozen.md)); the exact authoring-view bytes the LLMs were evaluated against are part of the research record and are preserved as `.taca` alongside the new canonical artifacts. This is the only directory class where `.taca` files are checked in.

**No global `*.taca` gitignore.** The default-no-checkin policy is enforced by convention (the LLM/IDE workflow doesn't produce these files in the first place), not by ignore rules. A repository-wide ignore would block the Phase 3 preservation case without buying anything else, since spurious `.taca` files don't appear under normal use.

## Work items

### 1. ADR for the reconciliation

- File: `decisions/0071-storage-format-reconciliation.md`.
- Acknowledges the Phase 1 drift as a spec bug against [ADR 0013](../decisions/0013-canonical-text-format-frozen.md) and [ADR 0014](../decisions/0014-sidecar-format.md), per the CLAUDE.md ground rule that "spec ambiguities are bugs against the relevant frozen artifact."
- Pins the three-extension end state: `.tac` = canonical, `.tacd` = JSON sidecar, `.taca` = transient authoring view.
- Notes that ADRs 0033, 0046, 0070 referenced "`.tac` files written in the authoring view" in passing. That wording becomes a historical artifact of the implementation drift; the freezes themselves stay intact — this reconciliation is implementation catching up to ADR 0013, not a freeze reversal.
- Must articulate the freeze-vs-repair distinction crisply so the reconciliation does not read as scope renegotiation.

### 2. Sidecar format upgrade (`.tac.sidecar.toml` → `.tacd`)

- The `.tacd` JSON parallel-tree schema is already specified in [sidecar-format.md](sidecar-format.md) (frozen by ADR 0014). It carries `binder`/`binders`, `comment`, `field_order`, plus the structural `children` array.
- Existing `.tac.sidecar.toml` schema (TOML, `[types.<binding>]` block per [ADR 0043](../decisions/0043-p2-test-conventions.md)) carries types and effects but lacks names/comments/field order.
- Extend the `.tacd` JSON schema with type and effect blocks (folded in from ADR 0043). Bump `tacd_version` if the existing readers can't tolerate the new keys; otherwise additive.
- Implement the parallel-tree reader/writer in `tacit-views::sidecar` (the crate already exists; today it reads TOML).
- Build a one-shot migration tool: `tacit migrate-sidecar <foo.tac> <foo.tac.sidecar.toml> -o <foo.tacd>`. Reads authoring `.tac` + TOML sidecar, emits canonical `.tac` + JSON `.tacd`. Used once across the repo, then deleted.

### 3. CLI surface

- `tacit canonicalize <foo.taca> [-o foo.tac]` — parse authoring view, emit canonical text plus `.tacd` sidecar. Hole nodes pass through; `--strict` rejects them.
- `tacit render <foo.tac> [--authoring | --inspection [--types --effects --debruijn --hashes]] [-o foo.taca]` — render the requested view from canonical. Default: authoring view to stdout.
- `tacit compile <foo.tac>` — reads canonical only. Convenience: also accepts `.taca` and runs `canonicalize` internally without persisting the canonical form.
- `tacit check <foo.tac>` — same as `compile` on input shape.
- `tacit view` — already exists; flips its default input from authoring to canonical.

### 4. Repository conversion

Mechanical pass; one PR or a small series, sequenced by directory. Two conversion modes apply.

**Mode A — preserve as historical record.** Used for Phase 3 research artifacts. The original authoring-view bytes are preserved verbatim as `.taca`; new canonical artifacts are generated alongside.

For each existing authoring `.tac` in this class:

1. Parse to AST.
2. Rename original `<name>.tac` → `<name>.taca` (preserved unchanged).
3. Rename paired `<name>.tac.sidecar.toml` → `<name>.taca.sidecar.toml` (preserved unchanged; keeps the historical pair adjacent and unambiguous).
4. Emit canonical text to `<name>.tac`.
5. Emit `<name>.tacd` JSON, folding type/effect data from the historical `.taca.sidecar.toml`.

Mode A targets:
- `corpus/<task>/reference.tac` and `corpus/<task>/reference.stdlib.tac` (47 open + 12 round-2 stdlib references). These were the falsification surface for [ADR 0070](../decisions/0070-p3-frozen.md).
- `examples/phase-3/*.tac` (3 carry-over programs: sort, list, sum-numbers). These closed [ADR 0046](../decisions/0046-p2-stage-5-frozen.md) § 3 and are explicit Phase 3 deliverables.

Each task directory in Mode A ends up with four files: `reference.taca` + `reference.taca.sidecar.toml` (historical) and `reference.tac` + `reference.tacd` (current). The eval harness reads the new canonical `reference.tac` for compile/check; result-record commentary that needs to cite "what the LLM was graded against" references the `.taca` file by name.

**Mode B — convert in place.** Used for test fixtures that are not Phase 3 research outputs. The original is replaced; no `.taca` is preserved.

For each existing authoring `.tac` in this class:

1. Parse to AST.
2. Emit canonical text → overwrite `.tac`.
3. Emit `.tacd` JSON, folding data from the paired `.tac.sidecar.toml`.
4. Delete the `.tac.sidecar.toml`.

Mode B targets:
- `examples/smoke/*.tac` (Phase 1 + Phase 2 + Phase 3 stdlib smoke programs). These are regression fixtures; their value is in compiling and producing expected output, not in their exact text.
- `plans/test-vectors/`: audit; convert any test vectors that are meant to be canonical reference inputs.

The Phase 3 stdlib smoke programs (`p3-*.tac` under `examples/smoke/`) are Mode B despite their Phase 3 origin: they are compiled-and-run smoke tests whose role is regression coverage, not falsification evidence.

### 5. Test and CI updates

- Round-trip tests in `tacit-views` already exercise authoring↔canonical via sidecar; flip them to read canonical `.tac` as the on-disk input and treat authoring as the rendered string.
- CI smoke step (`.github/workflows/ci.yml`): `tacit compile examples/smoke/hello.tac` continues to work because `.tac` is now canonical and `compile` reads canonical.
- Add a CI step that, for each example, renders authoring view from canonical and re-canonicalizes; assert hash stability across the round-trip.

### 6. Phase 3 corpus eval integration

The eval harness presents primer + task to LLMs. Primer teaches authoring view; the LLM emits authoring text; the harness parses and runs. Under the new world:

- LLM output (authoring text) → `canonicalize` → canonical AST → hash.
- Compare LLM output's canonical hash against `reference.tac` (canonical) for structural equivalence. Hash equality is a stronger correctness check than the text-equality fallbacks the current harness leans on.
- The primer itself is unchanged — it teaches the authoring grammar, which is how LLMs read and write Tacit. No `.taca` references needed in the primer text.

### 7. Documentation passes

- `CLAUDE.md` repository-layout section: `.tac` = canonical, `.tacd` = sidecar, `.taca` = transient authoring (not checked in).
- `plans/tacit-plan.md` § Storage format and § File organization: reflect the three-extension model; explicitly state that `.taca` is never stored in practice.
- `plans/canonical-text-format.md` § 0: strengthen the existing "not what humans or AI write" line by contrasting `.tac` vs. `.taca` directly.
- `plans/sidecar-format.md`: add type/effect blocks if absent; clarify that this is now the only sidecar format.
- Primer pass: ensure example snippets in markdown are framed as "authoring view," not "the contents of a `.tac` file." A small wording cleanup, not a structural edit.

### 8. Version control drivers (clarification, deferred)

- Structural diff/merge/blame drivers were always planned ([tacit-plan.md § Version control](tacit-plan.md)). They become implementable cleanly once `.tac` is canonical. Not delivered here, but unblocked by this work.
- Optionally land a thin `.gitattributes` rule (`*.tac diff=tacit-canonical`) registering a textconv that renders to inspection view, so even pre-driver `git diff` produces useful output.

## Sequencing

1. **ADR 0071** — gating prerequisite per CLAUDE.md ("write the ADR before writing the spec text or code that depends on it").
2. **Sidecar `.tacd` reader/writer + migration tool.** Implementation core.
3. **CLI surface** (`canonicalize`, `render`, updated `compile`/`check`/`view`).
4. **Repository conversion** — one mechanical pass once tooling is in place.
5. **Test/CI updates** — flip round-trip directionality, add round-trip-stability check.
6. **Documentation passes** — CLAUDE.md, plans, primer wording.

Items 2–3 are the implementation core; 4 is a one-time data migration; 5–6 are cleanup. Total scope: a focused multi-week effort, not a phase.

## Risks

- **LLM eval reproducibility.** Phase 3 baseline and Stage 10 result records reference specific reference solutions. Hash-stable migration preserves semantic equivalence, but any reference that fails to round-trip cleanly is a tooling bug to fix *before* migration. Mitigation: dry-run the migration tool on the full corpus and verify hash stability of every reference before committing the conversion PR.
- **Frozen-ADR optics.** Phase 1/2/3 freezes describe `.tac` as authoring view in passing. Per CLAUDE.md ground rules, freezes stay frozen. ADR 0071 must be unambiguous that this is implementation catching up to ADR 0013, not a freeze reversal. The spec-bug framing is the right one.
- **Phase 4 dependency.** If Phase 4 starts before this reconciliation completes, structural-edit and language-shape work built atop authoring-as-storage will need rework. Either reconcile first, or accept Phase 4 ships alongside this and structural-edit work waits for completion.
- **Mixed-state window.** During migration, some `.tac` files are canonical and some are authoring. Cleanest mitigation: convert atomically per directory; forbid mixed state in any committed tree. The `tacit compile` tool can sniff the first byte to detect format mismatch and fail loudly, but this is a transition aid, not a long-term feature.

## Out of scope

- **Object store implementation** ([tacit-plan.md § Object store](tacit-plan.md)) — still deferred to when cross-project imports actually materialize.
- **Custom Git diff/merge/blame drivers** — unblocked by this work, not delivered here.
- **AST-edit protocol** stretch target — Phase 4+.
- **Cross-project name overlay format** — Phase 4+ when imports arrive.
- **Authoring grammar changes.** The grammar from [ADR 0003](../decisions/0003-authoring-view-bpe-compact.md) stays; only its on-disk role changes.
- **Inspection view changes.** Already display-only and round-trip-exempt per [ADR 0015](../decisions/0015-inspection-view-scope.md); unaffected.
