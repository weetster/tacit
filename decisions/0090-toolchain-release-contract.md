# 0090 - Toolchain release contract

**Status:** Accepted
**Date:** 2026-05-17
**Phase:** Toolchain export, Stage 0 design
**Closes:** [toolchain-export-plan.md Stage 0](../plans/toolchain-export-plan.md)
**Amends:** [ADR 0082](0082-phase-6-package-manifest-lockfile-cache.md),
[ADR 0087](0087-phase-6-source-level-stdlib-foundations.md), and
[ADR 0089](0089-phase-6-frozen.md) additively.

## Context

Phase 6 made Tacit usable as a multi-file, package-aware language inside this
repository. The toolchain export work must now make a separate directory usable
with only an installed `tacit` command and its bundled release assets.

The release contract has to stay separate from existing content identities:
definition hashes identify canonical definitions, package hashes identify
ordinary Tacit package graphs, and a toolchain release identifies the compiler
binary plus the bundled assets an agent or human sees at development time.

No design, implementation, or validation work for the export may read, list, or
otherwise depend on `corpus/sealed/`.

## Decision

### Toolchain release manifest

A Tacit toolchain release has a deterministic JSON manifest named
`toolchain-release.json` with format `tacit-toolchain-release-v1`.

The manifest is not a package manifest and is not part of any package hash. It
describes an installed compiler binary and the exact bundled assets shipped with
that binary.

The first schema is:

```json
{
  "format": "tacit-toolchain-release-v1",
  "toolchain_version": "0.7.0",
  "git_rev": "...",
  "llvm": {
    "feature": "llvm19-1",
    "version": "19.1",
    "codegen": true
  },
  "schemas": {
    "canonical": "tacit-canonical-v1",
    "lockfile": "tacit-lock-v1",
    "package": "tacit-package-v1",
    "test_results": "tacit-test-v1",
    "interface": "tacit-interface-v1",
    "toolchain_release": "tacit-toolchain-release-v1",
    "toolchain_pin": "tacit-toolchain-pin-v1"
  },
  "assets": {
    "root": "share/tacit",
    "primer": {
      "id": "tacit-lite",
      "version": "0.7.0",
      "toolchain_version": "0.7.0",
      "path": "share/tacit/primer/tacit-lite.md",
      "metadata_path": "share/tacit/primer/tacit-lite.toml",
      "hash": "blake3:...",
      "tokenizer": "o200k_base",
      "tokens": 26265
    },
    "workflow": {
      "path": "share/tacit/workflow/agent-workflow.md",
      "hash": "blake3:..."
    }
  },
  "stdlib": [
    {
      "name": "tacit.core",
      "hash": "blake3:...",
      "source": { "registry": "builtin", "name": "tacit.core" },
      "source_path": "share/tacit/stdlib-src/tacit/core",
      "cache_path": "share/tacit/stdlib-cache/packages/<64-hex>"
    }
  ],
  "distribution": {
    "kind": "binary-archive",
    "layout": "tacit-toolchain-archive-v1"
  }
}
```

Serialization rules:

- JSON is UTF-8, two-space indented, emitted with the object key order shown
  above and a trailing newline.
- Hashes are written as `blake3:<64-lower-hex>`.
- `stdlib` entries are sorted by `name`, then by package hash bytes if names
  ever collide.
- Optional future fields are additive only after a new schema decision. Readers
  reject unknown fields for this schema.

The release hash is:

```text
release_hash = BLAKE3(toolchain-release.json bytes)
```

The release hash is reported by CLI commands and stored in project pins, but is
not embedded as a self-referential field inside `toolchain-release.json`.

### Bundled asset layout

The supported installed layout for the first export is prefix-style:

```text
<prefix>/
  bin/tacit
  share/tacit/
    toolchain-release.json
    primer/tacit-lite.md
    primer/tacit-lite.toml
    workflow/agent-workflow.md
    stdlib-src/tacit/{core,bytes,array,text,collections,io}/
    stdlib-cache/
      objects/
      packages/
    templates/executable/
    templates/library/
```

`share/tacit/stdlib-cache/` uses the cache layout from ADR 0082 without the
project-local `.tacit/cache/` prefix. `tacit stdlib seed` copies or hardlinks
from this cache-shaped tree into `<project>/.tacit/cache/`.

`share/tacit/stdlib-src/` is an inspected source snapshot for humans, agents,
and release verification. The cache objects are the authority for seeding and
hash-pinned builds. Release validation must prove that the source snapshot and
cache entries agree on the package hashes recorded in the manifest.

The binary embeds a byte-exact copy of the release manifest. The installed
`share/tacit/toolchain-release.json` must match the embedded bytes. If the
adjacent file is missing, `tacit version --format json` may still report the
embedded manifest and release hash, but `tacit doctor` reports an incomplete
installation and asset-dependent commands fail with a structured diagnostic.

Runtime asset lookup is:

1. `TACIT_TOOLCHAIN_ASSET_ROOT`, when set. It points directly at the
   `share/tacit/` directory containing `toolchain-release.json`.
2. `../share/tacit/` relative to the resolved `tacit` executable.

Source-tree tests may set `TACIT_TOOLCHAIN_ASSET_ROOT`. A source-tree build
without staged assets is not a release installation.

### Primer and toolchain matching

The first export requires exact primer/toolchain matching.

- `primer.toolchain_version` must equal `toolchain_version`.
- `primer.version` must equal `toolchain_version` for the first release line.
- The BLAKE3 hash in `primer/tacit-lite.toml` and in the release manifest must
  match the installed `primer/tacit-lite.md` bytes.
- `tacit primer --check <path>` compares the provided file's BLAKE3 hash with
  the installed primer hash. It does not accept compatibility ranges.

Any primer byte change, including prose-only edits, requires a new toolchain
patch release and therefore a new release manifest hash. A later ADR may define
compatibility ranges after the language and primer stabilize.

### Project pin file

Generated projects contain `tacit-toolchain.toml` at the project root. The file
is strict TOML with schema `tacit-toolchain-pin-v1`:

```toml
format = "tacit-toolchain-pin-v1"

[toolchain]
version = "0.7.0"
release_hash = "blake3:..."

[primer]
id = "tacit-lite"
version = "0.7.0"
toolchain_version = "0.7.0"
hash = "blake3:..."

[stdlib]
"tacit.core" = "blake3:..."
"tacit.bytes" = "blake3:..."
"tacit.array" = "blake3:..."
"tacit.text" = "blake3:..."
"tacit.collections" = "blake3:..."
"tacit.io" = "blake3:..."
```

The dotted stdlib package names are quoted keys. Unquoted dotted keys are not
accepted for this schema because they would create nested TOML tables instead
of exact package-name keys.

When a pin file is present, package-aware commands validate it before package
work:

- `toolchain.version` must equal the installed `toolchain_version`.
- `toolchain.release_hash` must equal the installed release hash.
- `primer` fields must match the installed primer metadata and hash.
- Every pinned stdlib package must match the installed release manifest.
- Unknown fields, missing required fields, malformed hashes, or type errors are
  errors.

Pin mismatches are hard errors for `check`, `compile`, `test`, `interface`, and
`lock`. The diagnostic must name the expected value from the project pin and
the installed value.

### Project templates

First-export templates generate canonical source plus sidecars only:

```text
src/main.tac
src/main.tacd
```

They do not generate `.taca` files. Authoring view remains transient per
ADR 0071, and the checked-in `.taca` files in this repository are historical
exceptions, not the model for new projects.

Generated projects ship both `AGENTS.md` and `CLAUDE.md`. They are generated
from the same release template and carry the same toolchain-facing guidance so
Codex-style agents and Claude Code both discover the project correctly. The
files instruct agents to fetch the matching language primer with `tacit primer`
and to use workflow guidance from the installed toolchain when tool use is
needed. They must not copy primer prose from this repository.

### Missing pin behavior

For the first export, a missing `tacit-toolchain.toml` is a warning, not an
error, for package-aware commands. This preserves existing repository fixtures,
examples, and manifestless projects while the export surface is introduced.

`tacit init` always writes a pin file. `tacit doctor --format json` reports pin
state as `present`, `missing`, or `mismatch`. A later ADR may promote missing
pins from warning to error after generated projects and migration tooling have
settled.

### First distribution channel

The first supported release vehicle is a reproducible binary archive containing
`bin/tacit` and the complete `share/tacit/` tree.

`cargo install` is not a supported release channel for the first export because
Cargo does not install the required primer, workflow, stdlib cache, source
snapshots, templates, and release manifest in the prefix layout above. Building
or installing from source remains a developer workflow, and `tacit doctor`
should distinguish it from a complete release installation.

## Alternatives considered

### Use package hashes as toolchain release identity

Rejected. Package hashes identify Tacit source package graphs. They do not
cover compiler version, LLVM support, primer bytes, workflow documents,
templates, or the bundled stdlib snapshot as an installed toolchain.

### Permit primer compatibility ranges

Rejected for the first export. The primer is part of the agent-facing language
contract, and Tacit is still changing quickly. Exact matching makes behavior
reproducible and forces prose changes to be released deliberately.

### Make missing project pins an immediate error

Rejected for the first export. That would force every existing fixture,
example, and manifestless project to adopt release pins before the export
commands are proven. Present-but-mismatched pins still fail hard, so generated
projects get strict behavior immediately.

### Support `cargo install` as an official first release path

Rejected. It would either omit release assets or require custom installation
logic that Cargo does not provide. A binary archive matches the desired
toolchain layout and can be checksummed as a complete unit.

### Ship only cache objects for stdlib

Rejected. Cache objects are sufficient for reproducible builds, but a separate
project also needs discoverable source metadata for humans and agents. The
source snapshot is advisory; the cache remains authoritative for seeded
hash-pinned builds.

### Include `.taca` files in project templates

Rejected. `.taca` is a transient authoring view. New exported projects should
start from canonical `.tac` files plus `.tacd` sidecars and let tools render
authoring or inspection views on demand.

## Consequences

- Stage 1 can implement one deterministic manifest generator and one release
  hash rule before adding user-facing commands.
- Stage 2 binds `tacit primer` to exact installed bytes and metadata.
- Stage 3 seeds bundled stdlib packages by copying from a cache-shaped
  `share/tacit/stdlib-cache/` tree.
- Stage 4 templates can write a complete `tacit-toolchain.toml` without
  waiting for later pin-policy decisions.
- Stage 5 validates present pins strictly while treating absent pins as a
  compatibility warning for the first export.
- Release packaging must assemble and verify the complete `share/tacit/` tree;
  a standalone binary is not a complete Tacit toolchain release.
