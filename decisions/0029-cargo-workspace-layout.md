# 0029 — Cargo workspace layout: `crates/` directory, `tacit-canonical` as first crate

**Status:** Accepted
**Date:** 2026-04-24
**Phase:** 1, Stage 1
**Closes:** deferred clause in [ADR 0016](0016-rust-ast-enum-location.md) — "Any Phase 1 consumer... a short ADR will decide..."

## Context

Phase 1 Stage 1 ([phase-1-plan.md § Stage 1](../plans/phase-1-plan.md)) requires:

> Introduce a top-level `Cargo.toml` workspace. Move the canonical-text crate
> into the workspace as `tacit-canonical`. ADR for the workspace layout (crate
> names, layering rules: `tacit-canonical` has no deps on view/codegen crates).

[ADR 0016](0016-rust-ast-enum-location.md) explicitly deferred the Cargo workspace to Phase 1, noting that a shared crate would be scaffolding for consumers that didn't yet exist during Phase 0. Phase 1 is that moment: the LLVM emitter and view crates are about to be written and all need the canonical AST.

### Design space

- **(A) `crates/` subdirectory, one crate per logical layer.** Conventional Rust workspace layout. Each Phase 1 crate lives under `crates/<name>/`. The `Cargo.toml` workspace member list grows as crates are added. Clear separation from the Python impl at `impls/py-canonicalizer/`.

- **(B) Flat root, crates at top level.** `tacit-canonical/`, `tacit-views/`, `tacit-codegen/`, `tacit-cli/` all sit at the repo root. Less nesting, but the repo root becomes cluttered with Rust-only artifacts at the expense of the spec files, plans, and corpus.

- **(C) Keep in `impls/rs-*` prefixed directories.** Extend the Phase 0 layout. Adding `impls/tacit-views/`, `impls/tacit-codegen/` alongside `impls/rs-canonicalizer/` is consistent with the Phase 0 "two impls" framing but treats the compiler pipeline as one more implementation rather than the primary artifact.

### Crate renaming

The existing package was named `tacit-canon` (a short abbreviation). At workspace promotion time it becomes the canonical dependency name for all other crates. `tacit-canonical` spells out the concept fully, matches the spec terminology ("canonical text"), and avoids any ambiguity with future `tacit-canon-law` humor.

### Layering rule

The plan states: "`tacit-canonical` has no deps on view/codegen crates." This prevents circular dependencies and keeps the canonical AST layer independent of consumers. Concretely:

```
tacit-cli
  ├── tacit-views
  │     └── tacit-canonical
  └── tacit-codegen
        └── tacit-canonical
```

`tacit-canonical` sits at the bottom; nothing it depends on may depend on it.

## Decision

**(A)** The workspace uses a `crates/` subdirectory. All Phase 1 Rust crates live under `crates/<crate-name>/`. The root `Cargo.toml` lists workspace members explicitly.

Concretely for Stage 1:

1. **Root `Cargo.toml`** at repo root declares `[workspace]` with `members = ["crates/tacit-canonical"]` and `resolver = "2"`. The workspace `Cargo.lock` lives at the repo root.
2. **`crates/tacit-canonical/`** is the promoted Phase 0 canonicalizer. Package name changes from `tacit-canon` to `tacit-canonical`; source files are identical except import paths in tests/binaries. The crate re-exports `ast`, `lex`, `parse`, `emit`, `hashing` at the crate root.
3. **`impls/rs-canonicalizer/`** is removed. The Python canonicalizer at `impls/py-canonicalizer/` is unchanged; `impls/` survives as the Python-impl home.
4. **Layering rule:** `tacit-canonical`'s `Cargo.toml` must never list `tacit-views`, `tacit-codegen`, or `tacit-cli` as dependencies. Violations require a new ADR.
5. **Future crates** added in Stages 2–5 follow the same `crates/<name>/` pattern and are added to the workspace member list in their respective Stage ADRs.

## Alternatives considered

- **Keep `impls/rs-canonicalizer/` in-place and add it to the workspace.** Would work mechanically (a workspace member can live anywhere), but leaves the "Phase 0 impl" framing in the path name and forces the workspace root to reach into `impls/`. Rejected in favour of a clean slate.
- **Name the crate `tacit-ast` or `tacit-core`.** Rejected. The crate today is the canonical-text parser+emitter+hasher. `tacit-ast` would be accurate for the `ast` module only; `tacit-core` is too vague. When Phase 2 introduces a separate type checker, the `tacit-canonical` crate boundary is already well-defined. A future `tacit-ast` that contains only the enum (without I/O) can be split off then if warranted.
- **Single monolithic `tacit` crate with feature flags.** Rejected. Feature flags are the wrong mechanism here: the codegen dependency on LLVM/`inkwell` must not infect the parser or the view renderer. Hard crate boundaries are cleaner.

## Consequences

- **`use tacit_canonical::...`** replaces `use tacit_canon::...` in all Rust consumers. The `tests/` files and the `dump-hashes` binary are updated as part of Stage 1.
- **CI** runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` from the workspace root (not from `impls/rs-canonicalizer/`). The cache key changes from `hashFiles('impls/rs-canonicalizer/Cargo.lock')` to `hashFiles('Cargo.lock')`.
- **The 38-vector byte-equivalence gate** continues to pass because the test logic is unchanged; only the crate name in the import changes.
- **Phase 1 Stages 2–4 crates** (`tacit-views`, `tacit-codegen`, `tacit-cli`) will each get a Stage ADR that adds them to the workspace member list. No ADR is needed for routine dependency additions within the already-accepted layering graph.
- **ADR 0016's referenced path** (`impls/rs-canonicalizer/src/ast.rs`) is now stale — the new canonical location is `crates/tacit-canonical/src/ast.rs`. ADR 0016 remains historically accurate for Phase 0; this ADR supersedes its packaging decision.

## Related decisions

- [ADR 0016](0016-rust-ast-enum-location.md) — Phase 0 decision to leave the AST in the canonicalizer; this ADR executes the deferred promotion.
- [ADR 0018](0018-stage-5-frozen.md) — Stage 5 CI freeze; the CI byte-equivalence gate is preserved by this ADR.
- [ADR 0013](0013-canonical-text-format-frozen.md) — canonical text format freeze; the format itself is unchanged.
- [phase-1-plan.md § Stage 1](../plans/phase-1-plan.md) — the Stage 1 deliverable this ADR closes.
