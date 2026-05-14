# 0081 - Phase 6 whole-project graph and deterministic derived layout

**Status:** Accepted
**Date:** 2026-05-14
**Phase:** 6, Stage 2
**Closes:** [phase-6-plan.md Q-P6-4](../plans/phase-6-plan.md)

## Context

ADR 0080 defined a logical `unit` artifact whose imports, exports, and
definitions are hash-addressed and independent of file layout. Stage 2 must
make a project load more than one `.tac` / `.tacd` pair without letting file
names, directory names, or file-system traversal order become semantic.

This is not package resolution yet. There is no manifest, lockfile, registry,
or dependency cache in Stage 2. The project graph is the local package-shaped
source set that Stage 3 and Stage 4 package work will later consume.

## Decision

Stage 2 project-aware commands accept a project root directory explicitly. A
file input remains a single-file command for backward compatibility. A later
package manifest may add implicit root discovery, but Stage 2 does not infer a
project root from arbitrary source files.

Within a project root, source discovery is deterministic:

- If `<root>/src` exists, it is the source base.
- Otherwise `<root>` is the source base.
- All `.tac` files below the source base are candidate canonical units.
- `.taca` files are transient authoring artifacts and are not project inputs.
- `.tacd` files are sidecars for a same-stem `.tac` file only.
- Derived output and tool state under `.tacit/`, VCS metadata under `.git/`,
  and build output under `target/` are ignored by source discovery.

Every project `.tac` file must parse to a canonical `unit` artifact. A
multi-file project is therefore a set of logical units, not a mixture of
single-expression programs and units. Existing single-file commands continue
to support expressions and old `module` binding groups.

Sidecars are advisory:

- Missing sidecars are allowed.
- Stale sidecars do not affect semantics and are ignored for aliases.
- Fresh sidecars provide display aliases for diagnostics and views.
- Duplicate aliases remain non-semantic; renderers and diagnostics fall back
  to hash-based names for ambiguous entries.

The project loader computes an in-memory hash index:

- `unit_hash = BLAKE3(canonical_text(unit))`.
- `definition_hash = BLAKE3(canonical_text(def))`, as defined by ADR 0080.
- Duplicate unit artifacts by `unit_hash` are coalesced.
- Duplicate definition artifacts by `definition_hash` are coalesced.
- Definition visibility is derived from all local export tables. `public`
  outranks `package`, and `package` outranks `private` for the same definition
  hash. Private definitions are still indexed so attempted imports can produce
  visibility diagnostics rather than path-dependent misses.

Traversal order is by hash, not by file path:

- Units are checked in ascending unit-hash order.
- Definition index entries are traversed in ascending definition-hash order.
- Import, export, and definition ordering inside each unit remains the
  canonical ordering from ADR 0080.

The deterministic derived layout is rooted at:

```text
<root>/.tacit/derived/project-<project-graph-hash>/
```

where:

```text
project-graph-hash =
  BLAKE3("tacit-project-v1\n" + sorted_unit_hash_0 + "\n" + ...)
```

The derived tree reserves these subdirectories:

- `units/<unit-hash>.tac` for canonical unit snapshots,
- `defs/<definition-hash>.tac` for canonical definition snapshots,
- `index/project-graph.json` for machine-readable graph metadata,
- `build/` for intermediate compiler outputs,
- `bin/` for final executables,
- `views/` for optional rendered authoring or inspection views.

The derived layout is cacheable output only. Deleting it must not change the
project's meaning.

Project-level `check` loads all units, builds the local hash index, verifies
imports and signatures through the existing unit checker, and reports
diagnostics against logical unit/import/definition hashes. Paths may appear as
source notes for humans, but a diagnostic's semantic identity is the sorted
unit position plus the relevant hash.

Project-level `compile` will use the same graph and must select an entry point
by public export hash or by a sidecar alias that resolves to one public export.
It must not select an entry by file path. The first Stage 2 implementation may
land project loading and `check` before compile support.

## Alternatives considered

### Path-based modules

Rejected. Treating `src/math/add.tac` as a module name would make directory
layout semantic and would conflict with ADR 0080's hash-addressed unit model.

### Relative-path imports

Rejected. `import "../math.tac"` is familiar, but it makes moving files a
semantic edit and weakens hash-based diagnostics. Tacit imports remain exact
definition hashes.

### Require a manifest before project loading

Rejected for Stage 2. The manifest and lockfile are Stage 3 decisions. Stage 2
needs a manifestless local graph so package design can build on real project
behavior instead of speculation.

### Fail on missing or stale sidecars

Rejected. Sidecars carry display metadata only. A missing or stale sidecar may
make diagnostics less readable, but it must not change type checking, hashes,
or compile behavior.

## Consequences

- Project-level behavior is deterministic for a fixed set of canonical unit
  bytes, independent of file names and directory ordering.
- Existing single-file commands remain compatible with older smoke programs.
- Package manifests can later point at the same project graph instead of
  inventing a second source-loading model.
- Compile support needs an explicit entry-selection rule based on public
  exports, not paths.
- No Phase 6 development work may use `corpus/sealed/` contents, paths,
  metadata, or feedback to validate this design.

## Related decisions

- [ADR 0013](0013-canonical-text-format-frozen.md) - canonical format.
- [ADR 0014](0014-sidecar-format.md) - sidecar metadata.
- [ADR 0071](0071-storage-format-reconciliation.md) - `.tac`, `.tacd`, and
  `.taca` file roles.
- [ADR 0079](0079-phase-6-scope.md) - Phase 6 scope and stage plan.
- [ADR 0080](0080-phase-6-module-semantics.md) - unit imports, exports,
  visibility, and hash semantics.
