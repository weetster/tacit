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

**Phase 3 corpus and carry-over programs are a deliberate exception.** They were the falsification surface for Phase 3 ([ADR 0070](../decisions/0070-p3-frozen.md)); the exact authoring-view bytes the LLMs were evaluated against are part of the research record and are preserved as `.taca` alongside the new canonical artifacts. This is the only directory class where `.taca` files are checked in *with paired canonical artifacts*.

**Model-generated files under `plans/phase-3-results/failures/` are also preserved as `.taca`, but with no canonical pair.** These are 482 LLM eval outputs (`generated.tac` per failure record) that are evidence of what the model emitted, not source. Most are failure cases that may not even parse cleanly. Canonicalizing them would (a) often be impossible without first repairing them, and (b) imply a stable canonical form for a transient artifact that doesn't merit one. They are renamed `.tac` → `.taca` and otherwise left untouched — no `.tacd` is produced, no canonical `.tac` is emitted alongside.

**No global `*.taca` gitignore.** The default-no-checkin policy is enforced by convention (the LLM/IDE workflow doesn't produce these files in the first place), not by ignore rules. A repository-wide ignore would block the Phase 3 preservation cases without buying anything else, since spurious `.taca` files don't appear under normal use.

## Work items

### 1. ADR for the reconciliation

- File: `decisions/0071-storage-format-reconciliation.md`.
- Acknowledges the Phase 1 drift as a spec bug against [ADR 0013](../decisions/0013-canonical-text-format-frozen.md) and [ADR 0014](../decisions/0014-sidecar-format.md), per the CLAUDE.md ground rule that "spec ambiguities are bugs against the relevant frozen artifact."
- Pins the three-extension end state: `.tac` = canonical, `.tacd` = JSON sidecar, `.taca` = transient authoring view.
- Notes that ADRs 0033, 0046, 0070 referenced "`.tac` files written in the authoring view" in passing. That wording becomes a historical artifact of the implementation drift; the freezes themselves stay intact — this reconciliation is implementation catching up to ADR 0013, not a freeze reversal.
- Must articulate the freeze-vs-repair distinction crisply so the reconciliation does not read as scope renegotiation.

### 2. Sidecar format upgrade (`.tac.sidecar.toml` → `.tacd`)

**Current state.** The `.tacd` JSON parallel-tree reader/writer already exists in `tacit-views::sidecar` (`SidecarNode` carrying `binder`/`binders`/`comment`/`field_order`/`children`, with `Sidecar` envelope handling `tacd_version` and `targets_hash_blake3`). The TOML reader lives separately in `tacit-typecheck::sidecar::TypeSidecar` and is consumed by `tacit-typecheck::sidecar::check_against_sidecar` (today's only call site for type/effect expectations). All 59 existing `.tac.sidecar.toml` files use only the `[types.main]` block — no per-binding entries are in flight, so the migration target is one type-hint + one effect-hint per program.

**Schema extension.** The `type_hint` and `effect_hint` keys are already reserved in [sidecar-format.md § 3.5](sidecar-format.md). Promote them to live keys: per-node, both optional, additive. No `tacd_version` bump (§ 2 already requires readers to ignore unknown keys). Today's TOML `[types.main]` migrates to `type_hint` + `effect_hint` on the *root* `display` node, where they describe the program's evaluated value type and effect set. Per-binding hints on child nodes are a future expansion path, not delivered here.

**Implementation steps:**

1. Add `type_hint: Option<String>` and `effect_hint: Option<Vec<String>>` to `tacit-views::sidecar::SidecarNode`. Update `is_empty` and round-trip tests.
2. Update [sidecar-format.md § 3.5](sidecar-format.md): move `type_hint` / `effect_hint` from "reserved" to a new live-keys subsection with worked example. The other reserved keys (`source_range`, `diagnostic_extra`) stay reserved.
3. In `tacit-typecheck::sidecar`, add `check_against_tacd(ast, &Sidecar) -> Result<(), Vec<Diagnostic>>` consuming `display.type_hint` / `display.effect_hint`. The existing `parse_type_str` / `parse_effect_list` helpers are reused unchanged.
4. Build the migration tool as a `tacit migrate-sidecar` subcommand (not a separate binary — keeps the workspace tidy). Inputs: paths to authoring `<foo.tac>` and `<foo.tac.sidecar.toml>`. Steps: parse authoring → AST + display sidecar; emit canonical bytes; merge type/effect from TOML into the root `SidecarNode`; write `<out>.tac` (canonical) and `<out>.tacd` (JSON). Flags: `--dry-run` (parse + canonicalize, report new hash, write nothing); `--in-place` (overwrite `<foo.tac>` and create `<foo.tacd>` next to it); `--strict` (reject hole nodes).
5. Once Item 4 (repository conversion) lands, retire `tacit-typecheck::sidecar::TypeSidecar` (TOML reader) and update `check_against_sidecar` callers to take `tacit_views::sidecar::Sidecar` instead. Keep `parse_type_str` / `parse_effect_list` helpers since they still parse the same value-language strings.
6. Delete the `tacit migrate-sidecar` subcommand after Item 4 commits successfully — it's a one-shot.

### 3. CLI surface

- `tacit canonicalize <foo.taca> [-o foo.tac]` — parse authoring view, emit canonical text plus `.tacd` sidecar. Hole nodes pass through; `--strict` rejects them.
- `tacit render <foo.tac> [--authoring | --inspection [--types --effects --debruijn --hashes]] [-o foo.taca]` — render the requested view from canonical. Default: authoring view to stdout.
- `tacit compile <foo.tac>` — reads canonical only. Convenience: also accepts `.taca` and runs `canonicalize` internally without persisting the canonical form.
- `tacit check <foo.tac>` — same as `compile` on input shape.
- `tacit view` — already exists; flips its default input from authoring to canonical.

**Implementation steps:**

1. Add `tacit canonicalize` subcommand. Reads input, parses with `tacit_views::authoring::parse_authoring`, emits canonical via `tacit_canonical::emit::emit`, writes `<out>.tac` and `<out>.tacd`. With no `-o`, derives output from input path by swapping `.taca` → `.tac`. Refuses to silently overwrite an existing canonical `.tac` (require `--force`). `--strict` rejects ASTs containing `Hole` nodes.
2. Add `tacit render` subcommand. Reads canonical via `tacit_canonical::parse::parse`, loads paired `.tacd` if present, renders authoring (default) or inspection (with the existing `--debruijn`/`--hashes`/`--types`/`--effects` flags). With no `-o`, prints to stdout; with `-o`, writes `.taca` and complains if the path doesn't end in `.taca`.
3. Refactor `tacit compile` to call a shared `load_canonical(path)` helper: if the path ends `.tac`, parse canonical directly; if `.taca`, parse authoring + canonicalize-in-memory (no persistence); else error. Retire the implicit "always authoring" assumption at `tacit-cli/src/main.rs:120`.
4. Same `load_canonical` helper for `tacit check` and `tacit view`. `view`'s `--as authoring` flag becomes a passthrough render; `--as inspection` keeps current behavior.
5. CLI integration tests: round-trip a small program through `canonicalize` → `render --authoring` → `canonicalize` and assert hash stability and disk shape (no stray files).

### 4. Repository conversion

Mechanical pass; one PR or a small series, sequenced by directory. Three conversion modes apply.

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

**Mode C — rename only.** Used for model-generated artifacts under `plans/phase-3-results/failures/`. The original is renamed `.tac` → `.taca`; no canonical `.tac` and no `.tacd` are produced.

For each existing `<run-id>/<task>/generated.tac` in this class:

1. Rename `generated.tac` → `generated.taca`.

That's the entire procedure. Justification:

- **These files are evidence.** They record what an LLM emitted under specific eval conditions. Modifying them (even to canonicalize losslessly) crosses a record-of-experiment line.
- **Many won't parse.** They are failure cases by definition; the `failures/` path means the run did not satisfy its task. Insisting on a canonical pair would force per-file repair or skip logic that buys nothing — the paired `diagnostics.json` already records *why* the file failed.
- **They are never compiled or rendered by tooling.** Nothing in CI or the eval harness reads `failures/**/generated.tac`; it lives there as a forensic record alongside `raw-response.txt` and `diagnostics.json`.

Mode C target:
- `plans/phase-3-results/failures/<run-id>/<task>/generated.tac` (482 files at the time of this plan).

**Implementation steps:**

1. Pre-flight (Modes A + B only): build a hash-stability dry-run script that, for each candidate file, parses authoring → canonicalizes → re-parses canonical → re-canonicalizes; asserts BLAKE3 stability and AST equality. Any failure here is a tooling bug fixed *before* the conversion PR. Mode C is exempt — no canonicalization runs.
2. Mode A pass over `corpus/<task>/reference{,.stdlib}.tac` (59 files): for each, run the `git mv .tac .taca && git mv .tac.sidecar.toml .taca.sidecar.toml`, then `tacit migrate-sidecar --in-place` against the original .taca to emit `.tac` + `.tacd`. Commit per directory subtree (algorithms / arithmetic / collections / io / strings) for reviewable diffs.
3. Mode A pass over `examples/phase-3/*.tac` (3 files: sort, list, sum-numbers). One commit.
4. Mode B pass over `examples/smoke/*.tac` (~30 files). For each: `tacit migrate-sidecar --in-place`, then `git rm` the `.tac.sidecar.toml`. One commit.
5. Audit `plans/test-vectors/`: identify any reference inputs meant to be canonical (likely already canonical in shape since they were authored against the spec); verify and convert if needed. One commit.
6. Mode C pass: bulk rename `find plans/phase-3-results/failures -name 'generated.tac' -exec sh -c 'git mv "$1" "${1%.tac}.taca"' _ {} \;` (or equivalent). Single commit, no other changes.
7. Run the full test suite plus the new round-trip-stability check (Item 5) before merging.

### 5. Test and CI updates

- Round-trip tests in `tacit-views` already exercise authoring↔canonical via sidecar; flip them to read canonical `.tac` as the on-disk input and treat authoring as the rendered string.
- CI smoke step (`.github/workflows/ci.yml`): `tacit compile examples/smoke/hello.tac` continues to work because `.tac` is now canonical and `compile` reads canonical.
- Add a CI step that, for each example, renders authoring view from canonical and re-canonicalizes; assert hash stability across the round-trip.

**Implementation steps:**

1. Update `tacit-typecheck/tests/smoke.rs` and `tacit-typecheck/tests/p3_carry_over.rs`: replace `parse_authoring(&src)` + `TypeSidecar::load(toml_path)` with `parse_canonical(&src)` + `Sidecar::read(tacd_path)`, then call the new `check_against_tacd`. Same coverage, new file shape.
2. Update `tacit-views/tests/round_trip.rs`: read canonical bytes from disk; render authoring; re-parse authoring; assert AST equal and re-canonicalize hash matches. (Today the test reads authoring and asserts canonical-of-parse round-trips; the new direction is canonical-as-source.)
3. Add a new integration test crate or test file: for every `.tac` under `examples/` and `corpus/`, parse canonical, render authoring, re-canonicalize, assert hash equal. This is the round-trip-stability gate.
4. Update `.github/workflows/ci.yml`: keep the existing `tacit compile examples/smoke/hello.tac` line (now reading canonical), and add `cargo test -p tacit-views --test round_trip_stability` (or whatever the test crate ends up named).
5. Delete fixture-loading paths that pointed at `.tac.sidecar.toml`. Grep for `sidecar.toml` post-migration; should be zero hits in `crates/`.

### 6. Phase 3 corpus eval integration

The eval harness presents primer + task to LLMs. Primer teaches authoring view; the LLM emits authoring text; the harness parses and runs. Under the new world:

- LLM output (authoring text) → `canonicalize` → canonical AST → hash.
- Compare LLM output's canonical hash against `reference.tac` (canonical) for structural equivalence. Hash equality is a stronger correctness check than the text-equality fallbacks the current harness leans on.
- The primer itself is unchanged — it teaches the authoring grammar, which is how LLMs read and write Tacit. No `.taca` references needed in the primer text.

**Implementation steps:**

1. In `corpus-eval` (Python), wherever LLM output is currently written for compilation, route it through a `tacit canonicalize --strict -` pipe (stdin → stdout) and capture the canonical bytes + hash.
2. Load `reference.tac`'s canonical bytes (post-Mode-A migration these *are* canonical) and compare hashes. On hash equality, mark "structural-equivalent."
3. Future failure-record writes save LLM output as `generated.taca` (not `.tac`) — aligns naming with the Mode C carve-out so the next eval round doesn't re-introduce authoring-view `.tac` files under `failures/`.
4. Existing `.run.json` schema fields (`structural_equivalent: bool`, etc.) accommodate the new check; verify no schema bump needed in `docs/phase-3-metrics.schema.json`.
5. The repair-loop, sealed-task, and result-label modes are unaffected — they don't touch storage shape.

### 7. Documentation passes

- `CLAUDE.md` repository-layout section: `.tac` = canonical, `.tacd` = sidecar, `.taca` = transient authoring (not checked in).
- `plans/tacit-plan.md` § Storage format and § File organization: reflect the three-extension model; explicitly state that `.taca` is never stored in practice.
- `plans/canonical-text-format.md` § 0: strengthen the existing "not what humans or AI write" line by contrasting `.tac` vs. `.taca` directly.
- `plans/sidecar-format.md`: add type/effect blocks if absent; clarify that this is now the only sidecar format.
- Primer pass: ensure example snippets in markdown are framed as "authoring view," not "the contents of a `.tac` file." A small wording cleanup, not a structural edit.

**Implementation steps:**

1. `CLAUDE.md` — repository layout block: add rows for `.tacd` and `.taca`; note the Mode A and Mode C `.taca` checked-in cases as exceptions.
2. `plans/tacit-plan.md` — § Storage format: pin the three-extension model. § File organization: list the conventions for paired `.tac`/`.tacd` and the carve-outs.
3. `plans/canonical-text-format.md` — § 0: contrast `.tac` (canonical, authoritative) with `.taca` (authoring, transient render).
4. `plans/sidecar-format.md` — promote `type_hint` / `effect_hint` from § 3.5 (reserved) to a new live-keys subsection with worked example. Note: ADR 0017 freezes the schema, so this addition is justified by ADR 0071 as the spec-bug-repair vehicle.
5. Primer pass (`plans/primer/tacit-lite-primer.md`): grep for "the contents of a `.tac` file" and similar; reframe as "authoring view." Pure wording cleanup; no structural change.
6. `decisions/0043-p2-test-conventions.md`: add a "Superseded by ADR 0071 for file format" note at the top, preserving the test conventions themselves.
7. `evaluation-harness-runbook.md`: update any references to `.tac.sidecar.toml` and authoring-as-stored to reflect the new flow.

### 8. Version control drivers (clarification, deferred)

- Structural diff/merge/blame drivers were always planned ([tacit-plan.md § Version control](tacit-plan.md)). They become implementable cleanly once `.tac` is canonical. Not delivered here, but unblocked by this work.
- Optionally land a thin `.gitattributes` rule (`*.tac diff=tacit-canonical`) registering a textconv that renders to inspection view, so even pre-driver `git diff` produces useful output.

## Sequencing

1. **ADR 0071** — gating prerequisite per CLAUDE.md ("write the ADR before writing the spec text or code that depends on it"). *Done; accepted 2026-05-06.*
2. **Sidecar schema extension + migration tool.** Add `type_hint`/`effect_hint` to `tacit-views::sidecar::SidecarNode`; promote them in `sidecar-format.md`; add `tacit migrate-sidecar` subcommand. Hash-stability dry-run mode lives here, used by Item 4.
3. **CLI surface** (`canonicalize`, `render`, updated `compile`/`check`/`view`). The `load_canonical(path)` helper is the central refactor; everything else is wiring.
4. **Repository conversion.** Run dry-run hash-stability gate first; convert in directory-scoped commits. Mode A (corpus + examples/phase-3), Mode B (examples/smoke), Mode C (failures rename), in that order. Mode C is independent and can land in parallel.
5. **Typecheck migration to `.tacd`.** Once fixtures are migrated, swap `tacit-typecheck`'s smoke and p3-carry-over tests to consume `.tacd`; retire `tacit-typecheck::sidecar::TypeSidecar` (TOML reader). Delete the migration subcommand.
6. **Test/CI updates** — flip round-trip directionality, add round-trip-stability check, ensure CI yml still works.
7. **Corpus-eval integration** — pipe LLM output through canonicalize, hash-compare against reference. Future failure outputs land as `.taca`.
8. **Documentation passes** — CLAUDE.md, plans, primer wording, ADR 0043 superseded note.

Items 2–3 are the implementation core; 4–5 are the one-time data migration; 6–8 are cleanup and integration. Total scope: a focused multi-day to multi-week effort, not a phase.

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
