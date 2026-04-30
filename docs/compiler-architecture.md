# Compiler Architecture

**Status:** Phase 2 complete. All five stages frozen; see
[ADR 0046](../decisions/0046-p2-stage-5-frozen.md) for Phase 2 freeze details.
LLVM 19 pinned via `inkwell` 0.9's `llvm19-1` feature.

## Crate layout

```
crates/
├── tacit-canonical/   — AST, lexer, parser, emitter, BLAKE3 hasher
├── tacit-views/       — authoring view (round-trip) + inspection view (display-only)
├── tacit-typecheck/   — Phase 2 structural type + effect checker (no LLVM dep)
├── tacit-codegen/     — Phase 1 LLVM IR emitter
└── tacit-cli/         — `tacit` binary: compile, check, and view subcommands
```

Dependency graph (arrows point from dependent to dependency):

```
tacit-cli ──► tacit-views      ──► tacit-canonical
    │
    ├────────► tacit-typecheck  ──► tacit-canonical
    │
    └────────► tacit-codegen   ──► tacit-canonical
```

No edges between `tacit-codegen` and `tacit-views` or `tacit-typecheck`.
`tacit-cli` is the only crate that depends on all three.

## `tacit-typecheck` layers

`tacit-typecheck` has no LLVM dependency and builds on any machine:

| Module         | Purpose                                                                        |
|----------------|--------------------------------------------------------------------------------|
| `ty`           | `Ty`, `EffSet`, `FnEff`, `Subst` — type and effect representation + unification. |
| `infer`        | Bidirectional inference pass; walks the AST and populates the substitution.    |
| `type_from_node` | Converts type-level AST nodes (`FnTy`, `TyVar`, `Forall`, `EffSet`, `EffVar`) to `Ty`. |
| `primitives`   | Builtin type and effect signatures for `@write`, `@read`, `@exit`, `@buf-alloc`, arithmetic. |
| `error`        | `Diagnostic` / `DiagOutput` — JSON-serialisable error format per ADR 0041.    |
| `sidecar`      | `.tac.sidecar.toml` type expectation loading and comparison (per ADR 0043).   |

Public entry point: `infer_module(node) -> Result<TypedModule, Vec<Diagnostic>>`.

Effect signatures for `@write`, `@read`, `@exit` are loaded from
[`stdlib/libc-effects.toml`](../stdlib/libc-effects.toml) at inference time
(schema frozen by [ADR 0025](../decisions/0025-phase-1-libc-surface.md)).

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
a binary that supports `tacit view` and `tacit check` but reports an error
for `tacit compile`.

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
    │  tacit_typecheck::infer_module(&node)
    │  (no LLVM dep; type/effect errors → JSON on stderr, exit 1)
    ▼
TypedModule  (or exit 1 on error)
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
    │  (failure → error message on stderr, exit 2)
    ▼
foo  (native executable)
```

The SidecarNode from parsing is discarded for `compile` — it carries
display metadata only and is not needed by codegen.

`--emit-llvm-ir` taps the pipeline just before object emission and prints
the textual `.ll` representation to stdout. Textual IR is an output only;
it is never read back in (ADR 0031).

**Exit codes:**
- `0` — success.
- `1` — type or effect errors (typechecker aborts before codegen).
- `2` — codegen or linker failure.

## `tacit check` pipeline

```
foo.tac  (authoring view bytes on disk)
    │
    │  tacit_views::authoring::parse_authoring(&src)
    ▼
(Node, SidecarNode)
    │ Node
    │  tacit_typecheck::infer_module(&node)
    ▼
TypedModule  — success (exit 0)
    or Vec<Diagnostic>  — errors (exit 1)

--format text  →  human-readable diagnostics on stderr
--format json  →  JSON DiagOutput envelope on stdout
```

`tacit check` has no LLVM dependency. It works in builds without any
LLVM feature flag.

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
        flags: InspectFlags { debruijn, hashes, types, effects } ▼
                                                  inspection text on stdout
```

Inspection flags:
- `--debruijn` (L1): appends `# x ≡ var N` annotations to variable references.
- `--hashes` (L2): prepends 4-byte BLAKE3 badges to each node.
- `--types` (Phase 2): renders type annotations (`FnTy`, `TyVar`, `Forall`) in
  human-readable form (e.g., `α0 -> Bool / {IO}`) instead of compact canonical.
- `--effects` (Phase 2): renders effect sets with spaces (`{IO, Mut}`) and
  effect variables as `ε0`.

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
by `tacit view` in Phase 2. Comment rendering in the inspection view is
possible when a sidecar is supplied programmatically (e.g., from a future
`tacit view --sidecar foo.tacd`); Phase 2 does not expose that flag.

## `tacit-cli` feature flags

`tacit-cli` mirrors `tacit-codegen`'s per-version LLVM feature flags:

```toml
# Cargo.toml / tacit-cli
llvm19-1 = ["tacit-codegen/llvm19-1", "llvm"]   # pinned; CI default
```

Building with a version feature enables the local `llvm` aggregate, which
gates the `compile_with_llvm_node` function in `main.rs`. Without any LLVM
feature, `tacit view` and `tacit check` work fully; `tacit compile` exits
with an error explaining how to rebuild.

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

## Phase 2 codegen additions

Phase 2 extended the lowered AST surface (Stage 4) while keeping the same
LLVM backend. These additions are covered by smoke programs #7 and #8.

| AST kind / pattern        | Lowering                                                              |
|---------------------------|-----------------------------------------------------------------------|
| `PatInt { value }`        | Integer-literal pattern arm: `icmp eq scrutinee, N` (ADR 0037).     |
| `Module { bindings }`     | Top-level bindings lowered identically to `Rec`; no `in` body.       |
| `@buf-alloc N`            | `alloca [N x i8]` — stack buffer (ADR 0038).                         |
| `@read fd buf len`        | `read(fd, buf*, len)` libc call; returns `i64` bytes read.           |
| `Ann { expr, type_ }`     | Transparent: the type annotation is consumed by the typechecker and  |
|                           | stripped before codegen; only `expr` is lowered.                     |

## Phase 3 codegen additions

Phase 3 Stage 2 added eight new `@name` primitives across four categories
(ADR 0047). All emit inline IR — no new external linkage except the
`llvm.memcpy.p0.p0.i64` intrinsic for `@buf-copy`.

| Category     | Primitive         | Arity | Effect     | Lowering                                        |
|--------------|-------------------|-------|------------|-------------------------------------------------|
| `STACK-ALLOC`| `@buf-alloc-dyn n`| 1     | `{Alloc}`  | `alloca i8, %n` (runtime size); let-RHS only.   |
| `MEM`        | `@buf-get buf off`| 2     | `{}`       | `gep i8, buf, off` + `load i8` + `zext i64`.   |
| `MEM`        | `@buf-set buf off byte` | 3 | `{Mut}`  | `trunc i8` + `gep i8, buf, off` + `store i8`. Returns 0. |
| `MEM`        | `@buf-copy dst doff src soff len` | 5 | `{Mut}` | `gep` both ptrs, call `llvm.memcpy.p0.p0.i64`. Returns 0. |
| `MEM`        | `@buf-eq a aoff b boff len` | 5 | `{}`   | Inline byte-compare loop; returns 0 or 1.       |
| `MEM`        | `@scan-byte buf off len target` | 4 | `{}` | Inline memchr-style loop; returns index or off+len. |
| `PARSE`      | `@parse-i64 buf off len` | 3 | `{}`    | Inline digit loop with optional leading `-`.    |
| `FORMAT`     | `@fmt-i64 buf off val` | 3 | `{Mut}`   | Digit-count pass + right-to-left write pass. Returns bytes written. |

Conformance tests for all eight primitives: `crates/tacit-codegen/tests/p3_primitives.rs`
(positive + boundary case per primitive). Source programs under `examples/smoke/p3-*.tac`.

Phase 3 Stage 4 also lifts the direct-call lowering from unary-only to closed
multi-argument lambda chains (ADR 0058). This is still not a closure ABI:
`lambda a. lambda b. body` lowers as one private function taking two `i64`
parameters, and callers must supply every argument at the call site.

Out of scope for Phase 1–3 (per the relevant ADRs):

- Open lambdas / first-class function values (ADR 0026).
- Records, projection, ctors as first-class values.
- Effect handlers, user-defined effects (Phase 7).

## See also

- [phase-2-plan.md](../plans/phase-2-plan.md) — Phase 2 plan + stage gates.
- [phase-1-plan.md](../plans/phase-1-plan.md) — Phase 1 plan (frozen).
- [ADR 0024](../decisions/0024-llvm-bindings-inkwell.md) — inkwell choice.
- [ADR 0025](../decisions/0025-phase-1-libc-surface.md) — libc surface + libc-effects.toml.
- [ADR 0026](../decisions/0026-phase-1-closed-lambdas.md) — closed lambdas.
- [ADR 0027](../decisions/0027-phase-1-rec-lowering.md) — rec lowering.
- [ADR 0028](../decisions/0028-phase-1-libc-call-surface.md) — `@name` surface.
- [ADR 0030](../decisions/0030-phase-1-arith-primitives.md) — arith/cmp intrinsics.
- [ADR 0031](../decisions/0031-llvm-distribution-and-self-hosting.md) — distribution + self-hosting.
- [ADR 0034](../decisions/0034-p2-type-subset-ann.md) — type subset for `ann`.
- [ADR 0035](../decisions/0035-p2-effect-set-canonical.md) — effect-set canonical syntax + lattice.
- [ADR 0036](../decisions/0036-p2-effect-polymorphism-syntax.md) — effect polymorphism surface.
- [ADR 0037](../decisions/0037-p2-pat-int.md) — `pat-int` canonical extension.
- [ADR 0038](../decisions/0038-p2-writable-buffer.md) — writable-buffer binding model.
- [ADR 0041](../decisions/0041-p2-structured-error-format.md) — structured error format.
- [ADR 0046](../decisions/0046-p2-stage-5-frozen.md) — Phase 2 freeze.
- [ADR 0047](../decisions/0047-p3-stdlib-expansion-surface.md) — Phase 3 `@name` surface expansion (PARSE/FORMAT/MEM + `@buf-alloc-dyn`).
- [phase-3-plan.md](../plans/phase-3-plan.md) — Phase 3 plan + stage gates.
