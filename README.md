# Tacit

Tacit is an AI-first programming language: a language designed for models to read and write, not for humans to type by hand.

The project starts from a different assumption than mainstream languages. If human readability is not the primary constraint, a language can optimize for three things at once:

- token efficiency for AI generation and consumption
- runtime performance via strong compile-time guarantees
- safety and security through explicit, structural semantics

Tacit compiles to LLVM IR and then native code. The current Tacit-Lite
compiler can parse, typecheck, inspect, compile, test, lock-package, and
execute the frozen Phase 6 language surface, and is shippable as a versioned
toolchain that drives Tacit projects outside this repository.

## What Makes Tacit Novel

- **The AST is the source of truth.** Tacit does not treat a human-oriented surface syntax as the authoritative program representation.
- **Programs have multiple lossless views.** A dense authoring view is optimized for AI token efficiency, while an inspection view is optimized for debugging and human review.
- **Canonical text is byte-exact.** Every valid AST has exactly one canonical serialization, which removes stylistic variance and formatter debates.
- **Definitions are content-addressed.** Functions, types, and values are identified by the BLAKE3 hash of their canonical text, so identity is structural rather than name-based.
- **Names are metadata, not identity.** Variable references use DeBruijn indices in canonical form; display names are advisory sidecar data.
- **Errors stay structural.** Malformed code becomes typed `Hole` nodes with structured diagnostics instead of opaque parse failures.
- **Effects are explicit.** Tacit-Lite tracks effects in function signatures so important behavioral facts remain visible without whole-program analysis.

## Design Direction

Tacit deliberately strips out many human conveniences:

- free-form formatting
- comments in source
- human-readable identifiers as semantic identity
- syntactic sugar and multiple spellings for the same construct
- prose-first error reporting

In exchange, it adds machinery that is useful for AI authoring:

- canonical AST storage
- purpose-built authoring and inspection views
- structural typing and explicit effect tracking
- content-addressed definitions and modules
- explicit recursion grouping and evaluation structure

The default target is **Tacit-Lite**, a smaller practical variant with structural types, simple effect tracking, and single-threaded execution. **Tacit-Full** is a longer-term research path that adds refinement types, capability-based security, and richer effect systems.

## Repository Guide

- `plans/` - project vision, phase plans, and frozen specs (canonical text format, inspection view, sidecar, toolchain export)
- `docs/` - supporting design notes (compiler architecture, effect system, installation)
- `decisions/` - ADR-style design decisions (0001 onward)
- `crates/` - Cargo workspace: `tacit-canonical`, `tacit-views`, `tacit-typecheck`, `tacit-codegen`, `tacit-cli`
- `examples/` - Phase 1 smoke corpus under `smoke/`, plus Phase 3, Phase 4, and Phase 6 examples (typed memory, fixed-int, data layout, package tests, embedding demo)
- `corpus/` - Phase 3 evaluation corpus, with sealed held-out subset
- `stdlib/` - source-level Tacit stdlib packages under `tacit/` (`core`, `bytes`, `array`, `text`, `collections`, `io`) plus `libc-effects.toml` for the typechecker
- `tools/` - one-shot generators and dev utilities
- `scripts/` - release scripts (e.g. `build-release.sh`)
- `share-assets/` - assets bundled into the toolchain `share/tacit/` tree (e.g. agent workflow)
- `impls/` - auxiliary implementations (e.g. `py-canonicalizer`)
- `tacit-toolchain-release.toml` - workspace-level release metadata pinning toolchain, primer, LLVM, and schema versions

## Current status

Phase 6 is frozen by [ADR 0089](decisions/0089-phase-6-frozen.md) and the
toolchain export plan completed on 2026-05-17 under
[ADR 0090](decisions/0090-toolchain-release-contract.md). The shippable
Tacit-Lite surface now includes:

- `unit` artifacts with hash-addressed imports/exports, visibility, and explicit boundary signatures (Phase 6)
- multi-unit project graphs with package manifests, lockfiles, a local hash-indexed cache, and package tests (Phase 6)
- fixed-width integers (`i8`/`u8` through `i64`/`u64`), wrapping/checked/saturating arithmetic, shifts, rotates, masks, byte-order helpers (Phase 6)
- typed mutable-memory handles, bounds-checked vector access, byte-bus load/store, and CPU/memory data-layout examples (Phase 6)
- source-level stdlib packages (`tacit.core`, `.bytes`, `.array`, `.text`, `.collections`, `.io`) consumed through ordinary package resolution (Phase 6)
- a constrained host-interface ABI with generated C headers and Rust bindings, plus a working Rust embedding demo (Phase 6)
- record products, first-class closures, and compiler-recognized `@map`/`@fold`/`@for-each` over `I64Vec` prefixes (Phase 4 carry-over)
- a versioned toolchain release with embedded release manifest, `tacit init`, `tacit-toolchain-pin-v1` enforcement, bundled stdlib seeding, and a Linux x86_64 GitHub Actions release pipeline (toolchain export)

Phase 5 (the bounded maintenance/debugging validation gate) is frozen by
[ADR 0078](decisions/0078-phase-5-decision.md), which accepted proceeding to
Phase 6 without a pre-Phase-6 tooling spike. **Phase 7 is the next planned
phase.** Debugger, diff/blame, IDE, public registry, arbitrary FFI, and broad
package-tooling work are deliberately out of scope until a later ADR reopens a
bounded slice.

Start with:

- `plans/tacit-plan.md` for the full project vision
- `plans/phase-6-plan.md` and `decisions/0089-phase-6-frozen.md` for the frozen Phase 6 baseline
- `plans/toolchain-export-plan.md` and `decisions/0090-toolchain-release-contract.md` for the toolchain release contract
- `docs/installation.md` for installing a published toolchain archive
- `CLAUDE.md` for the working rules used in this repo

## Trying Tacit in your own project

The toolchain is shippable as a self-contained Linux x86_64 archive. A new
project does not need to clone this repository, install LLVM, or know the
internal repo layout.

### 1. Install the toolchain

The first export targets **Linux x86_64**. The `tacit` binary links statically
against LLVM 19, so no LLVM runtime is required. Runtime deps (`libc`,
`libstdc++`, `libgcc_s`, `libm`, `libz`, `libzstd`, `libffi`) are present on a
default Debian-bookworm / Ubuntu-24.04 install.

Download `tacit-<version>-x86_64-unknown-linux-gnu.tar.gz` and its `.sha256`
companion from the release pipeline, then:

```sh
sha256sum -c tacit-<version>-x86_64-unknown-linux-gnu.sha256
tar -xzf tacit-<version>-x86_64-unknown-linux-gnu.tar.gz
sudo cp tacit-<version>-x86_64-unknown-linux-gnu/bin/tacit /usr/local/bin/
sudo cp -r tacit-<version>-x86_64-unknown-linux-gnu/share/tacit /usr/local/share/
tacit version --format json
```

For a per-user install with no root, use `~/.local/bin` and `~/.local/share`
instead. `tacit version --format json` should report
`installed_manifest.status: "matched"`. If you would rather build from source,
see `docs/installation.md`.

### 2. Create a project

In a directory outside this repository:

```sh
tacit init my-project --with-stdlib
cd my-project
```

`tacit init` writes a canonical project layout:

```
my-project/
  tacit-toolchain.toml   # pins toolchain, primer, and bundled stdlib hashes
  tacit.toml             # package manifest
  tacit.lock             # hash-pinned dependency lockfile
  AGENTS.md
  CLAUDE.md
  src/
    main.tac             # canonical text (authoritative)
    main.tacd            # JSON sidecar (display names, field order, hints)
```

`--with-stdlib` adds hash-pinned dependencies on the bundled stdlib packages
and seeds them into `.tacit/cache/`. Use `--template library` instead of the
default executable template if you want a library project that can emit a
host-interface library.

### 3. Check, test, compile

All package-aware commands verify the project's toolchain pin before doing any
work, so mismatches surface as structured `toolchain-pin-*` diagnostics:

```sh
tacit check .              # parse, canonicalize, typecheck, effect-check
tacit test . --format json # run package tests
tacit lock                 # regenerate tacit.lock after editing tacit.toml
tacit compile .            # build the executable (or library)
```

For a library project, `tacit interface . --emit-library` emits the C header
and Rust bindings for the constrained host-interface ABI. See
`examples/phase-6/embedding-demo/` for a Rust host program that links a Tacit
kernel as a static library.

### 4. Use the primer and workflow doc

Agents and humans can discover the exact bytes of the Tacit-Lite primer and
the agent workflow doc through the installed toolchain itself, so prose never
needs to be copied out of this repository:

```sh
tacit primer                        # print the primer markdown
tacit primer --format json          # primer id, version, hash, token count
tacit primer --check primer.md      # verify a copy matches the installed bytes
cat $(dirname $(which tacit))/../share/tacit/workflow/agent-workflow.md
```

The primer is pinned per [ADR 0090](decisions/0090-toolchain-release-contract.md),
so a published toolchain release always teaches the exact language surface it
implements.

For more detail (archive layout, environment variables, `tacit doctor`, source
builds), see [`docs/installation.md`](docs/installation.md).
