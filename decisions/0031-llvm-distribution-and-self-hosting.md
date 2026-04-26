# 0031 — LLVM distribution model and self-hosted-Tacit interaction

**Status:** Accepted
**Date:** 2026-04-25
**Phase:** 1, Stage 4 (with forward-looking Phase 6+ implications)
**Related:** [ADR 0022](0022-pure-kernel-host-model.md),
[ADR 0024](0024-llvm-bindings-inkwell.md)

## Context

[ADR 0024](0024-llvm-bindings-inkwell.md) committed Phase 1 to
`inkwell` as the LLVM binding layer and noted "the exact version
pair is chosen during Stage 4's familiarization spike." Drafting
Stage 4 surfaced three downstream questions ADR 0024 did not
answer:

1. **How does LLVM reach contributors and CI?** A source build
   (e.g., Homebrew compiling `llvm@18` from scratch) is on the
   order of hours and is sometimes the only option when the
   platform's package manager has no pre-built bottle. This is a
   recurring cost that compounds across contributors.
2. **How is the released `tacit` binary distributed?** A
   dynamically-linked `tacit` requires every user to have the
   matching LLVM version installed — unrealistic for a programming
   language's distribution story.
3. **What happens when Tacit becomes self-hosted?** The plan's
   Phase 6+ self-hosting milestone implies a Tacit-written compiler
   that needs to drive LLVM. Today's `inkwell` choice is a Rust
   convenience over LLVM-C; a self-hosted Tacit compiler would
   either need its own FFI or a different path to LLVM.
   [ADR 0022](0022-pure-kernel-host-model.md)'s "no FFI" rule was
   written for *user* code, but the compiler is privileged
   infrastructure and the question deserves a recorded answer.

These three questions were surfaced together by the Stage 4 work
and resolve cleanly as a single ADR rather than three small ones.

## Decision

### 1. Dev / CI dependency: pre-built LLVM, never source-build

Contributors and CI runners get LLVM via the platform's standard
mechanism, in this order of preference:

| Platform                | Path                                                                 |
|-------------------------|----------------------------------------------------------------------|
| Linux (Debian/Ubuntu)   | `apt install llvm-<N>-dev`                                          |
| Linux (Fedora/RHEL)     | `dnf install llvm<N>-devel`                                         |
| macOS (modern, with bottle) | `brew install llvm@<N>` — verify a bottle exists for the user's macOS version + arch before invoking |
| macOS (no bottle / older OS) | Pre-built tarball from `https://github.com/llvm/llvm-project/releases`, set `LLVM_SYS_<N><M>_PREFIX` |
| Windows                 | Pre-built MSVC tarball from same release page, set `LLVM_SYS_<N><M>_PREFIX` |

**Source builds are not a supported path.** A source build of LLVM
takes hours and is recoverable from a wedged state only by manual
process kills. If a contributor's platform has no pre-built option
for the chosen LLVM version, the resolution is one of:

- Switch to a supported platform / version pair.
- Use a Linux Docker container (`ubuntu:24.04` + `apt install
  llvm-<N>-dev`) for the build; not for the inner dev loop.
- Open a tracking issue to bump or downgrade the project's pinned
  LLVM version to one that *does* have pre-builts on the
  contributor's platform.

`docs/compiler-architecture.md` documents the per-platform commands
and includes a "before you install" sanity check (`brew info`,
`apt-cache policy`) that catches missing-bottle situations before
they cost hours.

### 2. Released-binary distribution: static linking

Released `tacit` binaries statically link the LLVM libraries the
codegen depends on. Users of the released compiler do **not** need
LLVM installed locally.

Concretely:

- Cargo build invocation for releases sets
  `LLVM_SYS_<N><M>_FORCE_LINK_DYNAMIC=0` (or equivalent) so
  `llvm-sys` (under `inkwell`) statically links every LLVM symbol
  it pulls in.
- Estimated released-binary size: **80–120 MB** (LLVM's IR builder,
  target backends, and a small linker entry compose the bulk).
  This is the price of removing a runtime LLVM dependency.
- Each `tacit` release version is bound to one LLVM version. Users
  do not see this — they install one binary, no LLVM dependency.
- LLVM bumps become a deliberate release-engineering task: bump the
  pin in `Cargo.toml` and `docs/compiler-architecture.md`, verify
  the smoke corpus on every supported platform, cut a release. Not
  Phase 1 work; documented here so it isn't a surprise later.

Pre-built `tacit` binaries are not produced in Phase 1. Phase 5's
"distribution" deliverable in [tacit-plan.md](../plans/tacit-plan.md)
is the first time CI-built artifacts ship; the static-linking
machinery lands then. Phase 1 contributors build from source against
a locally-installed LLVM (per § 1).

### 3. Self-hosted Tacit's path to LLVM: textual IR + `llc`

When the Tacit compiler is rewritten in Tacit (Phase 6+), the
self-hosted compiler **emits textual LLVM IR (`.ll`) and invokes
`llc` (or `clang`) as a subprocess** rather than FFI'ing into
LLVM-C.

Rationale:

- A self-hosted Tacit compiler that uses inkwell-shaped FFI would
  require [ADR 0022](0022-pure-kernel-host-model.md)'s "no FFI"
  rule to grow a compiler-infrastructure carve-out. The carve-out
  is defensible but opens a wedge: any tool authored in Tacit
  could argue for similar treatment.
- [ADR 0024](0024-llvm-bindings-inkwell.md) considered and rejected
  textual IR for *Phase 1* because the codegen abstraction would be
  rewritten when Phase 2's metadata work landed. That argument
  inverts for a self-hosted compiler: the codegen is being
  *rewritten anyway* (Rust → Tacit), so adopting a different IR
  representation at the same time has zero migration cost.
- Phase 10's "scratch stdlib replaces libc with direct syscalls"
  ([ADR 0022 § Phase boundary](0022-pure-kernel-host-model.md))
  already commits Tacit to the "shell out to OS facilities at the
  boundary" pattern. A self-hosted compiler that shells out to
  `llc` is consistent with that pattern, not a new architectural
  flavor.
- LLVM's textual IR is itself a stable surface across LLVM major
  versions for the small subset Phase 1 emits. Bump-skew between
  the self-hosted compiler and the installed `llc` is a less
  acute problem than between the inkwell crate version and the
  installed LLVM library version.
- Programmatic IR (`inkwell`) gives the Phase 1 Rust codegen
  better diagnostics on construction errors. A self-hosted Tacit
  codegen can recover the same property by structuring IR
  emission through a typed builder API in Tacit, *before* the
  text serialization step. The text becomes an output, not an
  abstraction layer.

The Phase 1 Rust codegen does not need to anticipate this
transition. The current `inkwell` choice is correct for Phase 1
and stays correct until Tacit takes over its own compilation.

The self-hosted-compiler IR-emission abstraction (Phase 6+) should
deliberately model `IRBuilder → Text` as the contract, with the
typed builder being Tacit-native. Inkwell is not directly
re-exposed; the textual IR is what crosses the boundary to `llc`.

## Alternatives considered

### Distribution: rely on user-installed LLVM (dynamic linking)

Rejected. Users of a programming language compiler should not need
to install LLVM separately, especially across the LLVM version-coupling
that `inkwell` enforces. Failure mode: user installs `tacit`,
gets a runtime "libLLVM.dylib not found" error, has to
reverse-engineer which LLVM version Tacit needs, install it
correctly, set library search paths. Compared to a static-linked
fat binary that just works, this is a category worse user
experience.

### Distribution: bundle LLVM dynamic libraries alongside the binary

Considered. Reduces the per-binary size by allowing OS-level
dedup of libLLVM across compiler versions. Rejected on
Phase-1 simplicity grounds: a single statically-linked binary is
the smallest moving-parts story and matches what `rustc` does on
non-Linux platforms. Phase 5 may revisit if binary size becomes a
release-blocking concern.

### Self-hosted: direct LLVM-C FFI from Tacit

Considered as the technically purest option — the same surface
`inkwell` wraps, just called from Tacit instead of Rust. Rejected
for two reasons: (a) it requires [ADR 0022](0022-pure-kernel-host-model.md)
to grow a compiler-infrastructure carve-out, weakening the
no-FFI commitment; (b) it commits the self-hosted compiler to
the inkwell-shaped programmatic-IR abstraction, which inherits
all of inkwell's version-coupling problems and gains nothing in
expressiveness over textual-IR-with-typed-builder.

### Self-hosted: keep LLVM but write the codegen as textual IR even in Phase 1

Considered. Avoids the Phase-1-Rust → Phase-6-Tacit
representation switch. Rejected: ADR 0024's rewrite-cost
argument still applies for Phase 2's metadata work; arriving at
Phase 2 with a string-based codegen abstraction would force a
mid-phase rewrite. The two-step path (`inkwell` for Phase 1,
textual IR for self-hosted) accepts a future rewrite at the
self-hosting boundary in exchange for Phase 2 ergonomics.

### Self-hosted: switch backend to Cranelift

Out of scope. [tacit-plan.md § Backend](../plans/tacit-plan.md)
commits to LLVM IR for the optimization story and WASM-target
candidacy; switching backends is a backend-architecture decision,
not a self-hosting decision, and would warrant its own ADR.

## Consequences

- **Dev-loop friction is bounded.** Pre-built LLVM means new
  contributors are minutes from a working build, not hours.
- **Released binary is large but self-contained.** Phase 5's
  release-engineering task accepts the 80–120 MB binary size as
  the cost of zero runtime LLVM dependency.
- **LLVM version bumps are scheduled.** A bump touches
  `Cargo.toml` (the pin), `docs/compiler-architecture.md` (the
  install commands), CI (the install step), and the smoke
  corpus (verification). Not zero-cost, but bounded and
  documented.
- **The self-hosted compiler is committed to a textual-IR
  emission path.** Phase 6+ planning can assume this; the
  intermediate "should we keep using inkwell from Tacit via
  FFI?" question is resolved.
- **`inkwell` becomes a Phase 1–5 choice, not forever.** The
  self-hosted compiler does not inherit the inkwell crate
  surface or its LLVM version-coupling. This bounds the
  long-term cost of the inkwell decision in ADR 0024.
- **`docs/compiler-architecture.md` gains a "Distribution" and a
  "Future: self-hosted" section.** Both land as part of Stage 5's
  documentation deliverable, not Stage 4.
- **The "no FFI" rule from ADR 0022 stays clean.** No carve-out
  for the compiler is needed; the self-hosted compiler's
  interaction with LLVM is a subprocess shell-out, which is the
  same pattern user code uses for OS-boundary symbols.

## Related decisions

- [ADR 0022](0022-pure-kernel-host-model.md) — pure kernel +
  host model; this ADR keeps the no-FFI rule intact by routing
  the self-hosted compiler's LLVM access through subprocess +
  textual IR.
- [ADR 0024](0024-llvm-bindings-inkwell.md) — Phase 1 inkwell
  choice; this ADR scopes that choice to Phase 1–5 and commits
  to a different path for self-hosted Tacit.
- [tacit-plan.md § Backend](../plans/tacit-plan.md) — names LLVM
  as the backend; unaffected.
- [tacit-plan.md § Phase 5 Distribution](../plans/tacit-plan.md)
  — first deliverable of pre-built `tacit` binaries; gains the
  static-linking commitment from this ADR.
- [tacit-plan.md § Phase 6+ Self-hosting](../plans/tacit-plan.md)
  — gains the textual-IR + `llc` commitment from this ADR.
- Future Phase 5 release-engineering ADR — may amend the
  static-linking story (e.g., bundled-dylibs alternative) when
  binary-size data exists.
- Future Phase 6+ self-hosted-codegen ADR — designs the typed
  Tacit IR builder whose serialization target is the textual
  LLVM IR pinned by this ADR.
