# Tacit Toolchain Export Plan

**Status:** Active; Stage 0 decision accepted 2026-05-17 by
[ADR 0090](../decisions/0090-toolchain-release-contract.md)
**Date:** 2026-05-17
**Scope:** Exporting Tacit as a versioned toolchain for separate Tacit projects

## Context

Tacit currently has the core pieces needed for external projects: a CLI that
can check, compile, test, render, canonicalize, lock packages, manage the local
cache, and generate host-interface artifacts; Phase 6 package and stdlib work
made source packages hash-addressed; and the Tacit-Lite primer is validated by
fixtures.

What is missing is a release contract. The compiler crates are local workspace
crates, stdlib package manifest versions are still development labels, and the
primer is a plan artifact under `plans/primer/` rather than a versioned
toolchain artifact. A separate project should not need to clone this repository
or know its internal layout in order to write, check, test, or compile Tacit.

This plan treats the export as a Tacit SDK/toolchain release. The toolchain
release owns the compiler binary, schema versions, bundled stdlib package
snapshot, primer, and project bootstrap templates.

No work under this plan may read, list, search, or otherwise access
`corpus/sealed/`.

## Goals

- Make a separate directory usable as a Tacit project with only an installed
  `tacit` toolchain.
- Version the primer and bind it to the toolchain release that it teaches.
- Make stdlib package use reproducible without path dependencies back into this
  repository.
- Let agents and humans discover the exact language primer, tool metadata, and
  stdlib hashes from the installed toolchain.
- Preserve Phase 6 content-addressed package semantics: canonical imports are
  exact definition hashes, and package dependencies are hash-pinned.
- Keep language-primer text separate from workflow/tooling guidance so recurring
  prompt cost stays measurable.

## Non-Goals

- No public package registry in the first export.
- No semantic-version dependency solver for Tacit packages.
- No implicit stdlib prelude or name-based `std` resolver.
- No change to canonical package identity: manifest names, descriptions,
  aliases, and human release labels remain advisory.
- No sealed-corpus evaluation or feedback.
- No requirement that Rust crates be published to crates.io in the first
  release. A binary/toolchain archive is enough if it is reproducible.

## Release Model

Add a single toolchain release identity. This is distinct from Tacit package
hashes:

```text
toolchain_version = human release label for compiler + bundled assets
package_hash      = BLAKE3 identity of a Tacit package graph
definition_hash   = BLAKE3 identity of one canonical definition artifact
primer_hash       = BLAKE3 identity of the exact primer bytes
```

The release manifest should be deterministic JSON, for example:

```json
{
  "format": "tacit-toolchain-release-v1",
  "toolchain_version": "0.7.0",
  "git_rev": "...",
  "llvm": "19.1",
  "schemas": {
    "canonical": "tacit-canonical-v1",
    "lockfile": "tacit-lock-v1",
    "package": "tacit-package-v1",
    "test_results": "tacit-test-v1"
  },
  "primer": {
    "id": "tacit-lite",
    "version": "0.7.0",
    "path": "share/tacit/primer/tacit-lite.md",
    "hash": "blake3:..."
  },
  "stdlib": {
    "tacit.core": "blake3:...",
    "tacit.bytes": "blake3:...",
    "tacit.array": "blake3:...",
    "tacit.text": "blake3:...",
    "tacit.collections": "blake3:...",
    "tacit.io": "blake3:..."
  }
}
```

The release manifest is not a package manifest. It describes the installed
toolchain and bundled assets. It should be embedded into the binary or shipped
next to it in a stable `share/tacit/` layout.

## Primer Packaging

Keep `plans/primer/tacit-lite-primer.md` as the development and historical
source of record during planning phases, but export a release copy as a
toolchain asset:

```text
share/tacit/primer/tacit-lite.md
share/tacit/primer/tacit-lite.toml
```

The metadata file should include:

```toml
id = "tacit-lite"
version = "0.7.0"
toolchain_version = "0.7.0"
hash = "blake3:..."
tokenizer = "o200k_base"
tokens = 26265
```

The first export should require an exact primer/toolchain match. A later ADR can
define compatibility ranges, but exact matching is safer while Tacit is still
changing quickly. Even a prose-only primer update should be a patch release,
because agent behavior depends on the exact bytes.

Add CLI support:

```text
tacit primer
tacit primer --format json
tacit primer --check <path>
```

Expected behavior:

- `tacit primer` prints the matching primer bytes.
- `--format json` prints primer id, version, hash, token count, and matching
  toolchain version.
- `--check` verifies a file's BLAKE3 hash against the installed primer metadata.

The current authoring primer should remain language-facing. Workflow guidance
for using `check`, `view`, diagnostics, repair loops, package tests, and future
debugging tools should be a separate asset:

```text
share/tacit/workflow/agent-workflow.md
```

That workflow document should be injected only when the task needs tool use.

## Stdlib Packaging

Bundled stdlib packages should remain ordinary Tacit packages. Exporting the
toolchain should not make them implicit.

The release process should compute and record package hashes for:

- `tacit.core`
- `tacit.bytes`
- `tacit.array`
- `tacit.text`
- `tacit.collections`
- `tacit.io`

Add a command that seeds the bundled stdlib package objects into a project cache:

```text
tacit stdlib seed
tacit stdlib seed --root <project>
tacit stdlib list --format json
```

Expected behavior:

- Materialize bundled stdlib units, definitions, sidecars, package indices, and
  manifest snapshots into `<project>/.tacit/cache/`.
- Print package names, hashes, public export aliases, and source metadata.
- Never rewrite user source files unless explicitly asked through `tacit init`
  or a future dependency-edit command.

External projects should use hash dependencies with optional advisory source
metadata:

```toml
[dependencies]
text = {
  hash = "blake3:...",
  source = { registry = "builtin", name = "tacit.text" }
}
```

This preserves Phase 6 behavior: the hash is the dependency, while names and
registries are hints.

## Project Pinning

Add a project-level toolchain pin file:

```text
tacit-toolchain.toml
```

Proposed format:

```toml
[toolchain]
version = "0.7.0"
release_hash = "blake3:..."

[primer]
id = "tacit-lite"
version = "0.7.0"
hash = "blake3:..."

[stdlib]
tacit.core = "blake3:..."
tacit.bytes = "blake3:..."
tacit.array = "blake3:..."
tacit.text = "blake3:..."
tacit.collections = "blake3:..."
tacit.io = "blake3:..."
```

Project commands should validate this file before doing package work:

- `tacit check`
- `tacit compile`
- `tacit test`
- `tacit interface`
- `tacit lock`

Mismatch diagnostics should be explicit:

- installed toolchain version differs from pinned version,
- installed primer hash differs from pinned hash,
- bundled stdlib package hash differs from pinned hash,
- pin file is missing and the command requires reproducibility.

Whether a missing pin is a warning or error should be decided by ADR. For the
first export, `tacit init` should always create the file.

## Project Bootstrap

Add:

```text
tacit init <name>
tacit init <name> --with-stdlib
tacit init <name> --template executable
tacit init <name> --template library
```

The generated project should be small and canonical:

```text
my-project/
  tacit-toolchain.toml
  tacit.toml
  tacit.lock
  AGENTS.md
  CLAUDE.md
  src/
    main.tac
    main.tacd
```

For an executable template:

- `tacit.toml` contains `[package]`, `[exports]`, and `[bin]`.
- `src/main.tac` contains one public executable entry returning `Int`.
- `tacit lock` succeeds immediately.
- `tacit check .` succeeds immediately.
- `tacit compile .` succeeds when the installed toolchain has LLVM support.

For a library template:

- `tacit.toml` contains `[package]` and `[exports]`.
- The exported definition has a scalar ABI-compatible shape where possible.
- `tacit interface --emit-library` can be demonstrated if the boundary is in
  the supported scalar subset.

The generated `AGENTS.md` and `CLAUDE.md` should come from the same release
template and instruct agents to use `tacit primer` to fetch the matching primer
instead of copying prose from this repository.

## CLI Surface

Add or extend CLI commands:

```text
tacit --version
tacit version --format json
tacit primer [--format json] [--check <path>]
tacit init <path> [--with-stdlib] [--template executable|library]
tacit stdlib list [--format json]
tacit stdlib seed [--root <path>]
tacit doctor [--format json]
```

`tacit doctor` should report:

- installed toolchain version,
- release manifest hash,
- LLVM/codegen support and pinned LLVM feature,
- C linker and archiver discovery,
- primer id/hash/version,
- bundled stdlib package hashes,
- project pin status when run inside a project,
- cache health summary.

The stable JSON output is important for agents. Human text output can remain
compact.

## Repository Work Items

### Stage 0: Decision Record

**Status:** Complete 2026-05-17. Deliverable:
[ADR 0090](../decisions/0090-toolchain-release-contract.md)

Write an ADR before implementation that decides:

- release manifest schema,
- exact primer/toolchain version matching,
- project pin behavior,
- location of bundled assets,
- whether first distribution is binary archive only or also `cargo install`,
- how missing `tacit-toolchain.toml` is handled.

### Stage 1: Release Metadata

**Status:** Complete 2026-05-17. Workspace release metadata, deterministic
manifest generation, embedded runtime metadata, adjacent-manifest verification,
and `tacit version --format json` are implemented.

- Add workspace-level release metadata.
- Add a deterministic release manifest generator.
- Embed release metadata and verify the adjacent installed manifest at runtime.
- Add `tacit version --format json`.
- Add tests that assert schema names and toolchain version are present.

### Stage 2: Primer Asset

**Status:** Complete 2026-05-17. The build generates the release primer copy
and metadata from `plans/primer/tacit-lite-primer.md`, records its BLAKE3 hash
and `o200k_base` token count, and `tacit primer` / `tacit primer --check` are
implemented with JSON metadata output.

- Add release primer asset generation from `plans/primer/tacit-lite-primer.md`.
- Compute BLAKE3 and token count.
- Add `tacit primer` and `tacit primer --check`.
- Extend primer fixture tests to validate the exported primer asset, not only
  the planning copy.

### Stage 3: Stdlib Bundle

**Status:** Complete 2026-05-17. The release manifest records bundled stdlib
package hashes and export metadata, the build stages cache/source stdlib
assets, and `tacit stdlib list` / `tacit stdlib seed` are implemented with a
hash-only external-project dependency test.

- Add a bundled stdlib package index to the release manifest.
- Add code to materialize bundled stdlib packages into a target cache.
- Add `tacit stdlib list` and `tacit stdlib seed`.
- Test that a temp external project can depend on a seeded stdlib package by
  exact hash without path dependencies into this repository.

### Stage 4: Project Init

**Status:** Complete 2026-05-17. `tacit init` generates pinned executable and
library projects with canonical source, sidecars, agent docs, optional bundled
stdlib dependencies, seeded cache entries, and deterministic lockfiles.
Integration tests cover generated-project `check`, `lock`, package tests,
executable compile, and library `interface --emit-library`.

- Add `tacit init`.
- Generate `tacit-toolchain.toml`, `tacit.toml`, starter units, sidecars,
  optional stdlib dependency entries, and lockfile. Do not generate `.taca`
  files.
- Add executable and library templates.
- Test that generated projects pass `check`, `lock`, and package tests.

### Stage 5: Toolchain Pin Enforcement

**Status:** Complete 2026-05-17. A strict `tacit-toolchain-pin-v1` parser
lives in `crates/tacit-cli/src/pin.rs` and is invoked from `tacit check` (in
project mode), `tacit compile` (in project mode), `tacit test`, `tacit
interface`, and `tacit lock`. Present-but-mismatched pins fail with structured
diagnostics (`toolchain-pin-{version,release-hash,primer,stdlib}-mismatch`,
`toolchain-pin-{schema-mismatch,schema-missing,missing-field,malformed,
stdlib-unknown,stdlib-missing,unreadable}`) naming the expected installed
value and the recorded pin value. A missing pin emits a single warning to
stderr referencing ADR 0090 and lets the command continue.

- Parse `tacit-toolchain.toml`.
- Validate it in package-aware commands.
- Add structured diagnostics for mismatches.
- Implement ADR 0090's first-export behavior: present mismatched pins are
  errors, while missing pins are warnings.

### Stage 6: Release Packaging

- Add a release script or CI workflow that builds the binary-archive
  distribution with the pinned LLVM feature, assembles `share/tacit/`, and
  emits checksums.
- Add an integration test that runs against an installed or staged toolchain
  outside this repository tree.
- Document installation and external project setup.

## Validation

Minimum release validation should create a temporary directory outside the repo
and run only through the staged toolchain:

```text
tacit init hello --with-stdlib
cd hello
tacit version --format json
tacit primer --format json
tacit stdlib list --format json
tacit check .
tacit lock
tacit test . --format json
tacit compile .
```

Additional validation:

- `tacit primer --check` accepts the bundled primer and rejects edited bytes.
- A project with an intentionally wrong `toolchain.version` fails clearly.
- A project with an intentionally wrong primer hash fails clearly.
- A cache-only stdlib dependency works without source paths into this repo.
- The exported primer's BLAKE3 hash and token count match release metadata.
- No validation step reads, lists, or searches `corpus/sealed/`.

## Stage 0 Questions

Resolved by [ADR 0090](../decisions/0090-toolchain-release-contract.md);
retained here as the original Stage 0 question list.

- Should `tacit check` reject missing `tacit-toolchain.toml`, or only warn until
  the first public release?
- Should the release manifest be embedded in the binary, shipped beside it, or
  both?
- Should stdlib bundle objects be embedded in the binary, shipped in
  `share/tacit/stdlib-cache/`, or generated from checked-in source packages at
  install time?
- Should primer patch releases be allowed without compiler crate version bumps,
  or should every primer change bump the whole toolchain?
- Should project templates contain authoring `.taca` files for readability, or
  only canonical `.tac` plus `.tacd` sidecars?
- What is the first supported installation path: binary archive, `cargo
  install --path`, Homebrew-style package, or all of the above?

## Success Criteria

This plan is complete when:

- An external project can be initialized and used without cloning this repo.
- The project records the exact toolchain, primer, and stdlib hashes it expects.
- The installed toolchain can print and verify the matching primer.
- Bundled stdlib packages can be consumed through ordinary hash-pinned package
  resolution.
- Release validation proves `check`, `lock`, `test`, `compile`, and `primer`
  flows in a temp external project.
- The release process records all hashes needed to reproduce the toolchain
  context seen by an agent.
