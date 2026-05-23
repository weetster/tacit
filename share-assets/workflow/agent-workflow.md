# Tacit Agent Workflow

This document is bundled with each Tacit toolchain release at
`share/tacit/workflow/agent-workflow.md`. It is the workflow companion to the
language primer (`share/tacit/primer/tacit-lite.md`) and is intended to be
injected only when an agent needs to use Tacit tools, not when an agent is only
reading or producing Tacit-Lite source.

The workflow doc, like the primer, is byte-pinned to the installed toolchain.
Treat it as part of the agent-facing contract for that toolchain release.

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

For a targeted language-syntax refresh, prefer selective primer disclosure
before loading the full primer:

```
tacit primer --search rec
tacit primer --list-sections
tacit primer --section helper-closure-and-callback-shapes
```

`tacit primer --search` uses case-insensitive plain substring matching, not
regex, so search alternation or pattern syntax literally will not work there.

Use `tacit primer` without filters when you need the complete language
contract.

## Starting or joining a project

For a new project:

```
tacit init my-project
tacit init my-project --template library
tacit init my-project --with-stdlib
```

`tacit init` writes `tacit-toolchain.toml`, `tacit.toml`, `tacit.lock`,
agent instruction files, and a canonical `src/main.tac` + `src/main.tacd`
pair. It never writes `.taca` files.

For an existing project: read `tacit-toolchain.toml` at the project root.
Mismatched pins are hard errors. Treat a missing pin as a request to run
`tacit init` or write the pin manually before invasive changes.

## The core authoring loop

### Editing source

`.tac` files are canonical S-expression bytes with BLAKE3 definition-hash
references; the primer teaches the authoring view (`.taca`), which is a
different surface syntax. Do not hand-edit `.tac`. Round-trip through
`tacit canonicalize` instead:

1. Render existing source as authoring view into the project's `.scratch/`
   directory: `tacit render <unit.tac> --as authoring -o .scratch/<scratch>.taca`.
   Create `.scratch/` if it does not exist; it is excluded by `.gitignore`.
2. Edit the scratch `.taca` using authoring-view syntax from the primer.
3. Canonicalize back into the project:
   `tacit canonicalize .scratch/<scratch>.taca -o <unit.tac> --force`. That
   rewrites both `<unit.tac>` and the `<unit.tacd>` sidecar.
4. Delete the scratch `.taca`. Do not check `.taca` files in.

Every source edit changes definition hashes. After step 3, run `tacit lock`
to refresh `tacit.lock`, then update any `[exports]`, `[bin]`, or `[[tests]]`
entries in `tacit.toml` that referenced the old hashes
(`tacit view <project-dir> --as inspection --hashes` or `tacit.lock` give
you the new ones).

### Validating

For validation, prefer this loop:

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

The host-interface library backend accepts scalar boundary types, ABI records,
and borrowed typed-vector parameters. Owned vector returns, vector fields
inside records, function values, legacy `Buf` / `I64Vec`, strings, and
arbitrary pointers remain outside the generated library ABI.

## Standalone executables and effects

Standalone-executable entries selected by `tacit compile .` must be `Int`,
`Bool`, or a fixed-width int. Function-typed entries are rejected with
`standalone executables require Int or Bool`.

The authoring-view definition form is `<alias> : <type> = <body>`. The
`/ {…}` effect annotation only attaches to a function arrow
(`A -> B / {…}`). There is no surface syntax for outer evaluation effects on
a value-typed definition, no `allow {…}` form, and no `[bin].effects`
manifest budget. Do not invent one.

A value-typed `main : Int` whose body calls effectful primitives
(`@buf-alloc`, `@buf-set`, `@fmt-i64`, `@write`, anything producing `Alloc`,
`Mut`, or `IO`) therefore cannot satisfy both `check` and `compile`. The
diagnostic signature is paired: `tacit check .` reports
`signature-mismatch: expected {}, got {…}` on the value-typed entry;
rewriting the entry as a function (`Int -> Int / {…}`) lets `check` pass but
then `tacit compile .` rejects it with the entry-type message above. Both
diagnostics together — not either one alone — indicate this gap.

The supported pattern for an effectful program is to expose the effectful
work as a function export and drive it from a host:

```
export public step : Int -> Int / {Alloc, IO, Mut} = ...
```

Run `tacit interface . --emit-library` to produce headers, bindings, and a
linkable archive; the host (typically a small Rust binary) satisfies host-
import callbacks and calls the public Tacit export. `tacit compile .` is
for pure transforms whose exit code is the whole result.

For Rust hosts, the generated `tacit_host.rs` also emits a per-package
callbacks trait so the host does not need to hand-write `unsafe extern "C"`
forwarders. Implement the trait and bind it once:

```rust
impl MyPkgCallbacks for MyHost {
    fn write_byte(&mut self, byte: u8) -> Result<i64, Error> {
        self.log.push(byte);
        Ok(byte as i64)
    }
}

let mut ctx = my_pkg_context { user: ptr::null_mut(), callbacks: ptr::null() };
ctx.bind_callbacks(MyHost::new());
```

The trait name is derived from the package's `tacit.toml` `[package].name`
(suffixed with `Callbacks`); packages with no host imports do not emit a
trait. Trait method names follow the host-import operation labels, so they
do not churn when source hashes change.

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
