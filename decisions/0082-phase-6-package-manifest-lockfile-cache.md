# 0082 - Phase 6 package manifest, lockfile, and dependency cache

**Status:** Accepted
**Date:** 2026-05-13
**Phase:** 6, Stage 3
**Closes:** [phase-6-plan.md Q-P6-5](../plans/phase-6-plan.md),
[phase-6-plan.md Q-P6-6](../plans/phase-6-plan.md)

## Context

ADR 0080 made the `unit` artifact the unit of cross-module composition, and
ADR 0081 made the project graph load multiple units deterministically without
giving file layout semantic weight. Stage 3 must now describe how a Tacit
package is identified, how its declared dependencies are recorded, how
historical resolutions are pinned, and how content-addressed artifacts are
stored on disk.

The scope-lock decisions from ADR 0079 constrain the shape of this stage:

- Dependency references must be hash-pinned. No semantic-version solver, no
  range syntax, no implicit "latest" lookup.
- No public registry operation. A registry alias is at most a name → hash
  lookup hint reserved for future tooling.
- Mutable display names (package name, dependency alias) must not gate the
  ability to rebuild a historical dependency closure.
- The sealed-corpus boundary applies: no Phase 6 design or validation work may
  read, list, or otherwise depend on `corpus/sealed/`.

The project graph from ADR 0081 already supplies a deterministic hash for a
manifestless local source set (`project-graph-hash`). Stage 3 extends that
model with an explicit package identity, declared external dependencies, and
a cache that persists units, definitions, and package indices across builds.

This ADR is a design ADR. Implementation lands in Stage 4 (manifest parsing,
lockfile management, cache wiring, structured diagnostics, fixtures).

## Decision

### Package identity

A package is the artifact described by a project root plus its declared
dependency resolution. Package identity is content-addressed:

```text
package_hash = BLAKE3(
  "tacit-package-v1\n" +
  sorted_unit_hash_0 + "\n" +
  sorted_unit_hash_1 + "\n" +
  ...
)
```

This is the same byte sequence used by `project-graph-hash` in ADR 0081 with
a different envelope tag. For any project graph, the package hash equals the
project-graph hash recomputed under the `tacit-package-v1` tag.

Consequences of this definition:

- A manifestless project produces a well-defined package hash. Adding a
  manifest that only carries display aliases, descriptions, or unused
  dependency hints does not change the package hash.
- Declared dependencies are not in the package hash directly. They participate
  only through the `imp` references already embedded in unit content (whose
  imported-definition hashes are part of each unit hash via ADR 0080).
- Renaming a package, changing its description, or changing local aliases does
  not change the package hash. These belong to advisory metadata.
- Adding, removing, or modifying a definition changes that definition's hash,
  which changes the containing unit hash, which changes the package hash.

### Manifest format

A package manifest is a TOML file named `tacit.toml` at the project root. An
absent manifest is allowed: the project is then treated as a manifestless
package whose hash is its project-graph hash and whose dependency table is
empty.

The manifest schema is additive. Unknown top-level tables or unknown keys are
rejected with `manifest-unknown-field`, matching the conservative
forward-compatibility rule from ADR 0080. Reader tools must not silently
ignore unknown fields in content-addressed configuration.

```toml
# tacit.toml — package manifest (advisory display + resolution hints)

[package]
# Display alias for this package. Advisory; not in package_hash.
name = "math"

# Optional human description. Advisory; not in package_hash.
description = "Numeric helpers"

# Optional human release label. Advisory; not used for resolution. No range
# syntax is interpreted.
version = "0.1.0"

[dependencies]
# Hash-pinned cache dependency. Local alias "core" is advisory.
core = { hash = "blake3:0123...cdef" }

# Local path dependency. Resolved each build from the path target's current
# project graph; locked into the lockfile by hash.
util = { path = "../util" }

# Optional registry alias hint. The hash field is still mandatory. The
# registry/name fields are passive metadata reserved for future tooling.
maths = {
  hash = "blake3:89ab...4567",
  source = { registry = "default", name = "tacit-maths" },
}

[exports]
# Optional stable consumer-facing aliases for public unit exports. Each value
# must be the hash of an exported public definition in this package. These are
# advisory display aliases at the package boundary, distinct from per-unit
# sidecar aliases.
double = "blake3:1234...abcd"

[bin]
# Optional executable entry points. Each value is either a public export
# hash, or an alias from [exports] that resolves to one. File names are not
# permitted as entry references.
main = "blake3:5678...ef01"
```

Rules:

- Every dependency entry must declare exactly one of `hash` (with optional
  `source` metadata), `path` (string), or future-reserved kinds (rejected
  now). Combined `hash` + `path` is rejected as `manifest-ambiguous-source`.
- `hash` values are written `blake3:<64-hex>`. Bare hex without the prefix is
  rejected.
- `path` values are relative to the project root. They are not glob
  patterns; one entry resolves to one project graph.
- `name`, `description`, `version`, and dependency aliases are advisory.
  Renaming any of them does not invalidate caches or lockfiles.
- Duplicate dependency aliases are rejected as `duplicate-dependency-alias`.
- A `[bin]` entry that does not resolve to a public export is rejected as
  `unresolved-entry`.
- Manifest text is parsed with strict TOML. Unknown keys, mistyped values, or
  schema-violating structures are rejected with `manifest-parse` or
  `manifest-unknown-field`.

The manifest is not hashed for package identity. Tools must not derive the
package hash from manifest bytes.

### Lockfile format

A lockfile is a JSON file named `tacit.lock` at the project root. It is
machine-managed: `tacit lock` (and the package-aware `tacit check` / `tacit
compile` commands) regenerate it from the manifest and the current state of
the cache and path dependencies.

The lockfile pins the resolution closure, not content. Content lives in the
cache and is content-addressed.

```json
{
  "format": "tacit-lock-v1",
  "package": {
    "hash": "blake3:..."
  },
  "dependencies": [
    {
      "alias": "core",
      "hash": "blake3:...",
      "source": { "kind": "cache" }
    },
    {
      "alias": "util",
      "hash": "blake3:...",
      "source": { "kind": "path", "path": "../util" }
    },
    {
      "alias": "maths",
      "hash": "blake3:...",
      "source": {
        "kind": "cache",
        "registry": "default",
        "name": "tacit-maths"
      }
    }
  ],
  "transitive": [
    {
      "hash": "blake3:...",
      "source": { "kind": "cache" }
    }
  ]
}
```

Deterministic serialization rules:

- Top-level keys appear in the fixed order `format`, `package`,
  `dependencies`, `transitive`.
- `dependencies` is sorted by declared alias.
- `transitive` is sorted by package hash bytes and contains every reachable
  package hash that is not a direct dependency, with no duplicates.
- Object keys inside `source` are sorted alphabetically.
- JSON is emitted with two-space indentation and trailing newline. Whitespace
  is fixed so the file is diffable and round-trips byte-exactly.

The lockfile is not content-addressed. Two different lockfiles can describe
the same package hash if their direct dependencies differ in alias only or in
registry-hint metadata.

### Resolution and lockfile drift

Resolution proceeds in this order for each declared dependency:

1. If `path` is present, load the path target as a project graph (ADR 0081)
   and compute its package hash. Source kind = `path`. The cache is updated
   with the resolved objects for reproducibility.
2. Otherwise, the dependency is a cache lookup. Source kind = `cache`. The
   referenced hash must be present in the cache (or fetchable by a future
   registry mechanism, which is out of scope here).

A lockfile is consistent with its manifest when, for each direct dependency:

- The resolved package hash matches the locked hash exactly, and
- The source kind matches.

`tacit check` and `tacit compile` reject mismatches with `lockfile-drift`.
`tacit lock` rewrites the lockfile from the current manifest plus resolution
results. A pure cache-only build with a consistent lockfile must succeed
without consulting any path target or external resolver.

Path-dependency drift is the common case: a developer edits a sibling
package, its package hash changes, and the consumer's locked hash no longer
matches. The diagnostic names the path target, the locked hash, and the
freshly observed hash, leaving the developer to either `tacit lock` or revert
the change.

### Local path dependencies

Path dependencies exist to support development across sibling packages
without a registry. They are not a long-lived release vehicle.

- Each path target must itself be a valid project root (per ADR 0081 source
  discovery).
- The path target is loaded as a fresh project graph each time resolution
  runs. Its package hash is computed and either matched against the lockfile
  or written into a new lockfile entry.
- Path-target objects (units, definitions, sidecars) are materialized into
  the cache so that downstream consumers see the same hash-addressed objects
  whether they came from a cache or a path.
- A path target may itself depend on other paths or cache entries. Path
  cycles in the dependency graph are rejected as `circular-package-dependency`.
- Path strings are not part of the package hash. Two manifests differing only
  in path strings can resolve to identical package hashes via different
  development trees.

### Object store layout

The dependency cache is hash-rooted. The default workspace-local cache is at
`<root>/.tacit/cache/`. The layout is content-addressed, immutable, and
shareable: nothing in this layout depends on file names supplied by the
project, on manifest text, or on alias metadata.

```text
<root>/.tacit/cache/
  objects/
    units/<64-hex>.tac          # canonical unit text, BLAKE3 == filename stem
    defs/<64-hex>.tac           # canonical definition text, BLAKE3 == stem
    sidecars/<64-hex>.tacd      # advisory unit sidecar; not verified by hash
  packages/
    <64-hex>/                   # one directory per package hash
      package.json              # regenerable index (see below)
      manifest.toml             # captured manifest at the time of resolution
      interface.json            # optional (Stage 10) ABI metadata; absent in Stage 3-9
  trash/                        # quarantined corrupt or evicted objects
```

A future change may add a shared user-level cache (for example, under
`$XDG_CACHE_HOME/tacit/`). The on-disk layout above is identical regardless
of root, so a workspace and shared cache may share the same `objects/` tree
via hardlinks. The location decision is implementation-level and deferred.

#### `package.json` index

`package.json` inside `packages/<hash>/` is a regenerable index of the
package's content. Its bytes are not part of the package hash, but its
content must be derivable solely from the unit set.

```json
{
  "format": "tacit-package-v1",
  "hash": "blake3:...",
  "units": [
    "blake3:...",
    "blake3:..."
  ],
  "public_exports": [
    "blake3:..."
  ],
  "package_exports": [
    "blake3:..."
  ]
}
```

Rules:

- `units` is sorted ascending by hash bytes and contains every unit hash
  that belongs to the package.
- `public_exports` is the sorted union of every `(exp public ...)` hash
  across the package's units.
- `package_exports` is the sorted union of every `(exp package ...)` hash;
  these are visible only inside the package and are still recorded here so
  internal tooling can answer "what does this package export" without
  rescanning every unit.
- `manifest.toml` is a verbatim copy of the project manifest at resolution
  time. It is not consulted during a cache-only build except as advisory
  display data; it may be absent for purely manifestless package snapshots
  produced from a project graph alone.

#### Atomicity and verification

- All cache writes are write-to-temp + fsync + rename within the cache root.
  No partial files are observable.
- Every read of `objects/units/<hash>.tac` and `objects/defs/<hash>.tac`
  recomputes BLAKE3 and verifies the file name. Mismatch is a
  `cache-corruption` diagnostic. The corrupt file is moved to `trash/` and
  the read is retried as a cache miss.
- `objects/sidecars/<hash>.tacd` is advisory. Sidecars are not hash-verified;
  they degrade gracefully under the ADR 0014 rules (stale = best-effort,
  missing = synthetic names).
- `packages/<hash>/package.json` is regenerable and re-verified by comparing
  its computed package hash against the directory name. Mismatch is a
  `cache-corruption` diagnostic and the package directory is rebuilt from
  the unit set in `objects/`.

#### Eviction

Eviction is explicit, not automatic, in Phase 6:

- `tacit cache clear` removes everything under `.tacit/cache/`.
- `tacit cache evict <hash>` removes objects and packages keyed by that hash.
- Garbage collection (drop entries no project graph references) is a Phase 7
  candidate and is not specified here.
- Eviction never affects lockfile content. A lockfile that points at an
  evicted object causes the next build to fail with `cache-missing-object`
  unless the object can be re-materialized from a path dependency.

### Registry aliases (out of scope, reserved)

A registry alias is a name → hash lookup. Phase 6 does not operate a
registry: there is no network protocol, no publish command, no version
solver, no transitive fetch. Manifests may carry optional `source.registry`
and `source.name` metadata as documented above; tools must accept these
fields, round-trip them in the lockfile, and otherwise ignore them.

A future ADR may define a registry implementation. When it does, it must
preserve two invariants from this ADR:

- A historical dependency hash remains buildable from the cache without
  consulting the registry.
- Registry lookups produce a hash; resolution and compatibility logic
  continues to operate on hashes only.

### Project-graph compatibility

The package model is an additive extension of the project graph from
ADR 0081, not a replacement.

- A project with no `tacit.toml` is a manifestless package. Its hash is the
  project-graph hash recomputed under `tacit-package-v1`. Its dependency
  closure is whatever its `imp` references resolve to inside the same
  project; cross-project imports require a manifest.
- A project with a `tacit.toml` is a manifested package. Its unit set, hash
  derivation, and derived-layout location remain identical. Its lockfile
  pins external dependency resolutions; its cache mirrors objects so the
  build is reproducible.
- Project-level `check` extends to include manifest parsing, lockfile
  verification, and dependency-closure resolution before the existing
  per-unit checker runs.
- Project-level `compile` continues to select entry points by public export
  hash or sidecar alias. The new `[bin]` table provides additional
  package-level entry aliases that resolve through the same mechanism.
- The derived layout under `.tacit/derived/project-<package-hash>/` (already
  rooted at the project-graph hash) is unchanged; the cache lives alongside
  at `.tacit/cache/` and is independent of derived outputs.

### Sidecars and interface metadata in cached packages

Sidecar storage in the cache obeys ADR 0014's advisory-only contract:

- Unit sidecars live at `objects/sidecars/<unit-hash>.tacd`. They are stored
  per unit hash, not per package, so two packages that share a unit share its
  sidecar. Conflicting sidecars for the same unit hash are resolved by
  preferring whichever is currently fresh (matching `targets_hash_blake3`);
  stale sidecars are ignored without diagnostic.
- Package-level display data lives in `packages/<hash>/manifest.toml`. There
  is no separate `package.sidecar.json`; the manifest is the source of
  user-supplied display metadata.
- Stage 10 (host-interface ABI) will write generated interface descriptions
  to `packages/<hash>/interface.json`. Stage 3 reserves the file name and
  defines no schema for it. Until Stage 10, the file is always absent.
- Generated C headers, Rust bindings, and other derived host-interface
  artifacts live under `.tacit/derived/...`, not in the cache. The cache
  stores canonical objects and package indices only.

### Diagnostics

Stage 3 reserves the following structured diagnostic kinds. Stage 4
implements them.

| Kind | Severity | Meaning |
| --- | --- | --- |
| `manifest-parse` | error | `tacit.toml` is not valid TOML or violates the schema. |
| `manifest-unknown-field` | error | An unknown top-level table or key was found in `tacit.toml`. |
| `manifest-ambiguous-source` | error | A dependency entry declares more than one of `hash`, `path`, or future-reserved kinds. |
| `manifest-missing-source` | error | A dependency entry declares neither `hash` nor `path`. |
| `duplicate-dependency-alias` | error | Two dependency entries share an alias. |
| `unresolved-entry` | error | A `[bin]` or `[exports]` entry does not resolve to a public export. |
| `lockfile-parse` | error | `tacit.lock` is not valid JSON or violates the schema. |
| `lockfile-drift` | error | The lockfile does not match the manifest's current resolution (hash mismatch, source mismatch, or missing entry). |
| `dependency-unresolved` | error | A declared dependency is not present in the cache and has no path. |
| `cache-corruption` | error | A cached object failed BLAKE3 verification or a package index disagreed with its directory name. |
| `cache-missing-object` | error | A locked hash references an object absent from the cache and not recoverable from any path dependency. |
| `circular-package-dependency` | error | The package dependency graph (cache or path) contains a cycle. |

Each diagnostic must carry the package alias (when known), the relevant
`blake3:<hash>` values, and the file path (for path-related cases). Aliases
are advisory; hashes are the stable repair target, matching ADR 0080.

### LLM-facing design constraints

The manifest, lockfile, and cache surface follow the same explicit-structure
preferences as ADR 0080:

- Dependency declarations name a hash. Resolution is a lookup, never a
  search.
- Lockfile rows are machine-managed and deterministic. A repair loop can
  regenerate them without losing information.
- Display aliases are decoupled from identity. Renaming a package or a
  dependency alias is a sidecar-level edit and never breaks builds.
- Diagnostics emit both aliases and hashes. Models can navigate by alias and
  repair by hash.
- Cache objects are content-addressed and verified on read. A poisoned cache
  fails loudly rather than producing silent wrong builds.

### Test-vector expectations

Stage 3 commits to these vector classes. Stage 4 supplies the fixtures.

Manifest vectors:

- A minimal manifest with `[package]` only.
- A manifest with cache, path, and registry-hinted dependencies.
- A manifest with `[exports]` and `[bin]` entries that resolve to public
  exports.
- Negative: ambiguous source, missing source, duplicate alias, unknown
  field, unresolved entry.

Lockfile vectors:

- A lockfile for a manifestless package (`dependencies` and `transitive`
  empty).
- A lockfile for a package with one cache dependency and one path
  dependency.
- A lockfile with non-empty transitive closure, sorted deterministically.
- Negative: lockfile drift from path mutation, lockfile drift from manifest
  edit, parse failure.

Cache vectors:

- Round-trip: write an object, read it back, hash matches.
- Corruption: tampered object file fails verification and moves to `trash/`.
- Eviction: explicit eviction removes objects; subsequent build fails with
  `cache-missing-object` unless re-materialized from a path.
- Package index regeneration: deleting `package.json` and rebuilding from
  `objects/` produces a byte-identical index.

## Alternatives considered

### Hash the manifest into package identity

Rejected. Including manifest bytes would make trivial display edits
(renaming the package, editing a description) invalidate every consumer's
locked hash. The package's *content* is its units; the manifest is
resolution and display metadata. Two manifests producing the same unit set
must collide on the same package hash.

### Use semantic-version ranges with a solver

Rejected by ADR 0079. A solver would reintroduce the long tail of
version-resolution failure modes (yanked versions, incompatible upgrades,
transitive conflicts) without earning expressive power that hash pinning
lacks. Hash pinning makes historical builds trivially reproducible.

### Make the manifest a canonical Tacit artifact

Rejected for Stage 3. Defining a `(package ...)` canonical node would touch
ADR 0013 yet again and grow the canonical-text surface for what is
fundamentally configuration data. Manifests are user-edited TOML; their
content addresses are derived from the units they reference, not from the
manifest bytes themselves.

### Per-package object store

Rejected. Putting `objects/units/<hash>.tac` inside each
`packages/<hash>/units/` would duplicate identical canonical bytes across
every package that references a shared unit. A hash-rooted global
`objects/` tree deduplicates naturally and matches the content-addressed
model.

### Verify sidecars by hash

Rejected. Sidecars carry display metadata only and may legitimately drift
relative to their canonical text. ADR 0014's `targets_hash_blake3` field is
the existing staleness signal; verifying sidecars as content-addressed
objects would conflict with their advisory role.

### Automatic cache eviction (LRU, time-based)

Rejected for Phase 6. Automatic eviction is a Phase 7 candidate once
multi-package workflows produce enough data to motivate a policy. Phase 6
ships explicit eviction commands only.

### Allow `extern` or system-path dependencies

Rejected by ADR 0079's non-goals. Tacit source must not name arbitrary
system libraries through the package manifest. Host-provided capabilities
arrive via the Stage 10 ABI, not through the dependency graph.

## Consequences

- Stage 4 implements manifest parsing, lockfile management, cache I/O,
  resolution, and the structured diagnostics reserved here.
- The Stage 1 and Stage 2 single-project flow continues to work. A
  manifestless project's package hash is exactly its project-graph hash
  under the new envelope tag.
- A historical lockfile plus a populated cache is sufficient to rebuild a
  package without consulting any registry, path target, or external service.
- Display edits to the manifest do not invalidate caches or lockfiles.
- Stage 5 (unit testing) and Stage 10 (host-interface ABI) consume the
  package boundary defined here: tests run against package-scoped public
  exports, and ABI metadata is rooted at `packages/<hash>/interface.json`.
- A future registry ADR may add a network protocol on top of the hash-keyed
  cache without changing the cache layout or the lockfile schema.
- No Phase 6 work may use `corpus/sealed/` contents, paths, metadata, or
  feedback to validate this design.

## Related decisions

- [ADR 0013](0013-canonical-text-format-frozen.md) - canonical text format.
  This ADR does not add canonical node kinds.
- [ADR 0014](0014-sidecar-format.md) - sidecar metadata; cached sidecars
  follow its advisory rules unchanged.
- [ADR 0022](0022-pure-kernel-host-model.md) - pure computational kernel;
  host paths and capabilities are explicitly out of scope here.
- [ADR 0079](0079-phase-6-scope.md) - Phase 6 scope, non-goals, and ADR
  sequence.
- [ADR 0080](0080-phase-6-module-semantics.md) - unit imports, exports,
  visibility, and definition-hash semantics. Package hashes are derived from
  unit hashes defined here.
- [ADR 0081](0081-phase-6-project-graph.md) - whole-project graph and
  derived layout. Package identity reuses the project-graph hash formula
  under a new envelope tag.
