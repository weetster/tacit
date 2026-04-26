# Compiler Architecture

**Status:** Phase 1 complete. Stages 1–5 frozen; see
[ADR 0032](../decisions/0032-stage-4-frozen.md) for Stage 4 freeze details.
LLVM 19 pinned via `inkwell` 0.9's `llvm19-1` feature.

## Crate layout

```
crates/
├── tacit-canonical/   — AST, lexer, parser, emitter, BLAKE3 hasher
├── tacit-views/       — authoring view (round-trip) + inspection view (display-only)
├── tacit-codegen/     — Phase 1 LLVM IR emitter
└── tacit-cli/         — `tacit` binary: compile and view subcommands
```

Dependency graph (arrows point from dependent to dependency):

```
tacit-cli ──► tacit-views ──► tacit-canonical
    │
    └────────► tacit-codegen ──► tacit-canonical
```

No edges between `tacit-codegen` and `tacit-views`. `tacit-cli` is the only
crate that depends on both.

## `tacit-codegen` layers

Two layers, with the LLVM-touching one gated behind feature flags:

| Module          | LLVM dep | Purpose                                                                  |
|-----------------|----------|--------------------------------------------------------------------------|
| `analysis`      | no       | Pure AST checks: closed-lambda check, hole check, App-spine unfolding, integer-literal parsing. |
| `error`         | no       | `CodegenError` enum; structured diagnostics for both layers.            |
| `primitives`    | no       | `@name` allowlist (LIBC ∪ ARITH ∪ CMP) and arity table.                |
| `compile`       | **yes**  | `inkwell`-based IR construction, object-file emission, target-machine setup. |

The split lets the analysis layer compile and test on any machine
without an installed LLVM. The IR-emission layer is built only when a
per-version feature is enabled:

```bash
# Without LLVM — analysis layer only:
cargo build -p tacit-codegen
cargo test -p tacit-codegen

# With LLVM (pick one matching your installed library):
cargo build -p tacit-codegen --features llvm19-1   # pinned version
cargo build -p tacit-codegen --features llvm18-1
cargo build -p tacit-codegen --features llvm15-0
```

`tacit-cli` mirrors this pattern — building without an LLVM feature produces
a binary that supports `tacit view` but reports an error for `tacit compile`.

## LLVM dependency

Per [ADR 0024](../decisions/0024-llvm-bindings-inkwell.md), the IR
emitter uses `inkwell` over LLVM-C. Per
[ADR 0031](../decisions/0031-llvm-distribution-and-self-hosting.md):

- **Dev / CI:** install LLVM from the platform's package manager, never
  source-build. See "Installing LLVM" below.
- **Released binaries:** statically link LLVM into `tacit` so users
  don't need LLVM installed. Release-engineering work, scheduled for
  Phase 5.
- **Self-hosted Tacit (Phase 6+):** emit textual `.ll` and shell out to
  `llc`. The Phase 1–5 inkwell choice is bounded; the self-hosted
  compiler does not inherit it.

### Pinned LLVM version

**LLVM 19**, `inkwell` feature flag `llvm19-1` (inkwell 0.9).

Rationale: LLVM 19 is the newest version available in Debian bookworm's
default apt repos (`llvm-19-dev`) without adding a third-party source.
It is also available as a brew bottle for arm64 macOS. inkwell 0.9 is
the first release to support LLVM 19 via the `llvm19-1` feature.

Contributors pass `--features llvm19-1` to build the IR emitter.
CI installs `llvm-19-dev` via apt (see `.github/workflows/ci.yml`).

### Installing LLVM (dev-loop)

| Platform                       | Command                                                                 |
|--------------------------------|--------------------------------------------------------------------------|
| Ubuntu / Debian                | `apt install llvm-<N>-dev`                                              |
| Fedora / RHEL                  | `dnf install llvm<N>-devel`                                             |
| macOS (modern, x86_64 or arm64)| `brew info llvm@<N>` *first* — verify a bottle exists for your macOS.<br>Then `brew install llvm@<N>`. |
| macOS (no bottle for your OS)  | Pre-built tarball from `https://github.com/llvm/llvm-project/releases`. |
| Windows (MSVC)                 | Pre-built MSVC tarball from the same GitHub releases page.              |

After install, set `LLVM_SYS_<N><M>_PREFIX` to point at the install
root if `llvm-config` isn't on `PATH` (the `inkwell`/`llvm-sys` build
script discovers LLVM through this env var). For brew installs:

```bash
export LLVM_SYS_191_PREFIX="$(brew --prefix llvm@19)"
```

### Pre-flight bottle check

**Always** verify a pre-built option exists before installing on macOS:

```bash
brew info llvm@19 | grep -A 2 'bottle:'
```

If only an `arm64` line is listed and you're on Intel — or if no
bottle line appears for your macOS major version — switch to the
LLVM.org tarball or use a different LLVM version. Source builds take
hours and are not a supported path
([ADR 0031 § 1](../decisions/0031-llvm-distribution-and-self-hosting.md)).

## `tacit compile` pipeline

```
foo.tac  (authoring view bytes on disk)
    │
    │  tacit_views::authoring::parse_authoring(&src)
    ▼
(Node, SidecarNode)
    │ Node
    │  tacit_codegen::compile_to_object / compile_to_ir_string
    │  (LLVM-gated; analysis::check_no_holes runs first)
    ▼
inkwell::Module
    │
    │  TargetMachine::write_to_file  (object emission, in-process)
    ▼
foo.o  (temp file)
    │
    │  system linker (cc / clang / gcc)
    ▼
foo  (native executable)
```

The SidecarNode from parsing is discarded for `compile` — it carries
display metadata only and is not needed by codegen.

`--emit-llvm-ir` taps the pipeline just before object emission and prints
the textual `.ll` representation to stdout. Textual IR is an output only;
it is never read back in (ADR 0031).

## `tacit view` pipeline

The view pipeline shares the parse step with compile but branches into
two separate renderers instead of codegen.

```
foo.tac  (authoring view bytes on disk)
    │
    │  tacit_views::authoring::parse_authoring(&src)
    ▼
(Node, SidecarNode)
    │           │
    │           │  SidecarNode: binder names, field order,
    │           │  comments (advisory metadata extracted from
    │           │  the authoring source text)
    │           │
    ├── --as authoring ──────────────────────────────────────────┐
    │   tacit_views::authoring::emit_authoring(node, sidecar)    │
    │                                                            ▼
    │                                              authoring text on stdout
    │
    └── --as inspection ─────────────────────────────────────────┐
        tacit_views::emit_inspection(node, sidecar, flags)       │
        flags: InspectFlags { debruijn, hashes }                 ▼
                                                  inspection text on stdout
```

### How the sidecar flows through `tacit view`

`parse_authoring` reconstructs display metadata from the authoring text as it
parses: binder names (variable names from `lambda x.`, `let x =`, pattern
variables) become `SidecarNode::binder` entries; `rec` group names become
`SidecarNode::binders`; record field authoring order is recorded in
`SidecarNode::field_order`.

For `--as authoring`: `emit_authoring` reads these entries to reproduce the
original names rather than synthetic `v0`, `v1`, … fallbacks. The result is a
normalised authoring-view text that round-trips through canonical form with
byte-identical hashes.

For `--as inspection`: `emit_inspection` reads the same entries to label
variable occurrences and binders; the `--debruijn` flag (`InspectFlags.debruijn`)
appends trailing `# x ≡ var N` annotations, and `--hashes`
(`InspectFlags.hashes`) prepends 4-byte BLAKE3 badges.

An external `.tacd` sidecar file (comments, additional metadata) is not loaded
by `tacit view` in Phase 1. Comment rendering in the inspection view is
possible when a sidecar is supplied programmatically (e.g., from a future
`tacit view --sidecar foo.tacd`); Phase 1 does not expose that flag.

## `tacit-cli` feature flags

`tacit-cli` mirrors `tacit-codegen`'s per-version LLVM feature flags:

```toml
# Cargo.toml / tacit-cli
llvm19-1 = ["tacit-codegen/llvm19-1", "llvm"]   # pinned; CI default
```

Building with a version feature enables the local `llvm` aggregate, which
gates the `compile_with_llvm` function in `main.rs`. Without any LLVM
feature, `tacit view` works fully; `tacit compile` exits with an error
explaining how to rebuild.

## Phase 1 codegen subset

| AST kind              | Lowering                                                                   |
|-----------------------|----------------------------------------------------------------------------|
| `Int`                 | `i64` constant.                                                            |
| `Var(i)`              | DeBruijn lookup in the binder stack.                                       |
| `Let { rhs, body }`   | If `rhs` is `Lam`: hoist + bind as `Function`. Otherwise: bind `i64` value. |
| `If`                  | `icmp ne cond, 0` + conditional branch + phi at merge (ADR 0030 truthy).  |
| `App` (closed `Lam` head)  | Hoist lambda + direct call (ADR 0026).                                |
| `App` (`Sym` head ∈ allowlist) | Primitive emit (libc call, arith op, or cmp + zext) (ADR 0028, 0030). |
| `App` (`Var` head → `Function`) | Direct call.                                                       |
| `Rec`                 | Forward-declare every member, define each body, then lower rec body (ADR 0027). |
| `Match`               | Chain of `icmp eq` arms; trailing `pat-wild` is the merge fallthrough.    |

Out of scope for Phase 1 (per the relevant ADRs):

- Open lambdas / first-class function values (ADR 0026).
- Hole-node recovery (ADR 0023).
- `Module` top-level (deferred per phase-1-plan.md Stage 2 exclusions).
- Records, projection, ctors as first-class values.
- `pat-int` patterns (canonical extension required for smoke #7).
- Writable-buffer binding model (required for smoke #8).

## See also

- [phase-1-plan.md](../plans/phase-1-plan.md) — phase plan + stage gates.
- [ADR 0024](../decisions/0024-llvm-bindings-inkwell.md) — inkwell choice.
- [ADR 0025](../decisions/0025-phase-1-libc-surface.md) — libc surface.
- [ADR 0026](../decisions/0026-phase-1-closed-lambdas.md) — closed lambdas.
- [ADR 0027](../decisions/0027-phase-1-rec-lowering.md) — rec lowering.
- [ADR 0028](../decisions/0028-phase-1-libc-call-surface.md) — `@name` surface.
- [ADR 0030](../decisions/0030-phase-1-arith-primitives.md) — arith/cmp intrinsics.
- [ADR 0031](../decisions/0031-llvm-distribution-and-self-hosting.md) — distribution + self-hosting.
