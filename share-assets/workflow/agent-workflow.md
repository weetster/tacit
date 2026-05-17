# Tacit Agent Workflow

This document is bundled with each Tacit toolchain release at
`share/tacit/workflow/agent-workflow.md`. It is the workflow companion to the
language primer (`share/tacit/primer/tacit-lite.md`) and is intended to be
injected only when an agent needs to use Tacit tools, not when an agent is only
reading or producing Tacit-Lite source.

Per ADR 0090 the workflow doc, like the primer, is byte-pinned to the
installed toolchain: any change to its bytes requires a new toolchain release
hash. Treat it as part of the agent-facing contract.

## What the installed toolchain looks like

A complete installation places one binary under `bin/` and a single
asset tree under `share/tacit/`:

```
<prefix>/
  bin/tacit
  share/tacit/
    toolchain-release.json
    primer/tacit-lite.md
    primer/tacit-lite.toml
    workflow/agent-workflow.md
    stdlib-src/tacit/{core,bytes,array,text,collections,io}/
    stdlib-cache/{objects,packages}/
    templates/{executable,library}/
```

The compiler embeds a byte-exact copy of `toolchain-release.json`. `tacit
doctor` and `tacit version --format json` will report a mismatch if the
installed copy and the embedded copy diverge.

## Discovering toolchain identity

Always confirm the toolchain you are running before producing changes:

```
tacit version --format json
tacit primer --format json
tacit stdlib list --format json
```

The `release_hash` from `tacit version` is the same value a project pin
records under `[toolchain].release_hash`. Use that hash to decide whether a
project's `tacit-toolchain.toml` matches the installed toolchain.

## Starting or joining a project

For a new project:

```
tacit init my-project
tacit init my-project --template library
tacit init my-project --with-stdlib
```

`tacit init` writes `tacit-toolchain.toml`, `tacit.toml`, `tacit.lock`,
`AGENTS.md`, `CLAUDE.md`, and a canonical `src/main.tac` + `src/main.tacd`
pair. It never writes `.taca` files.

For an existing project: read `tacit-toolchain.toml` at the project root.
Mismatched pins are hard errors; a missing pin is a warning for now per
ADR 0090, but you should treat it as a request to run `tacit init` (or to
write the pin manually) before invasive changes.

## The core authoring loop

For code-level work, prefer this loop:

1. `tacit check .` — type and effect check the whole project graph.
2. `tacit lock` — refresh `tacit.lock` after any dependency or visibility
   change.
3. `tacit test . --format json` — run package tests. Targets must be `Bool`
   definitions; declared effects must be a subset of the manifest's allowed
   effects.
4. `tacit compile . --emit-llvm-ir` to inspect generated IR, or
   `tacit compile . -o ./out` to produce a native executable. The
   `--entry <alias>` flag selects a public export by sidecar alias or hash.

For host integration:

```
tacit interface .                    # write interface.json + C header + Rust bindings
tacit interface . --emit-library     # additionally produce a linkable .a
```

The host-interface layer accepts only the scalar boundary types documented in
the primer's host-interface section. Records and borrowed vectors at the
boundary are rejected by `--emit-library` even when `interface.json` accepts
them.

## View tools

```
tacit view <path> --as authoring
tacit view <path> --as inspection [--debruijn --hashes --types --effects]
tacit render <path.tac> --as authoring -o <path.taca>
tacit canonicalize <path.taca> [-o <path.tac>] [--strict]
```

`tacit view <project-dir> --as inspection` is the project-wide inspection
view; `--as authoring` is not supported for projects. The `.taca` authoring
view is transient: never check it in to a new project, and never let a model
edit a `.tac` file directly — round-trip through `tacit canonicalize` if
authoring-view edits are needed.

## Diagnostics and exit codes

All package-aware commands emit `DiagOutput` JSON on stderr with
`--format json`. Common kinds you should recognize:

- `toolchain-pin-*` — project pin disagrees with the installed toolchain.
  Fix the pin or update the toolchain; do not silently rewrite source.
- `unresolved-import`, `hash-mismatch`, `signature-mismatch`,
  `visibility-violation`, `cyclic-dependency`, `duplicate-*` — package graph
  problems; fix the offending unit or its imports.
- `test-signature-mismatch`, `test-effect-violation`, `test-compile-failure`,
  `test-runtime-error` — surface in `tacit test` output. Treat all four as
  blocking; do not ship a passing-looking summary with errors above it.
- `abi-*` — host-interface generation rejected a boundary; trim the export
  surface or simplify the boundary type.

Exit codes: `0` for success, `1` for `fail` test outcomes, `2` for
`compile-fail`, `effect-fail`, or `error` test outcomes, `1` for any other
hard failure.

## Hand-off etiquette

Before handing changes back, in this order:

1. `tacit primer --check share/tacit/primer/tacit-lite.md` (or equivalent
   sanity check) if you ever wrote a primer copy locally.
2. `tacit lock` to refresh the lockfile.
3. `tacit check . --format json` and confirm `errors: []`.
4. `tacit test . --format json` and confirm `outcome: "pass"`.
5. `tacit compile .` if LLVM support is available and the project has a
   binary entry.

Do not run experimental tooling against `corpus/sealed/` if such a directory
exists in the host project — it is the held-out evaluation set in upstream
Tacit and must remain unread by the agent.
