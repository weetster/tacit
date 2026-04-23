# 0016 — Rust AST enum: spec-conformant, in-place in the canonicalizer

**Status:** Accepted
**Date:** 2026-04-22
**Phase:** 0, Stage 3
**Location of the enum:** [impls/rs-canonicalizer/src/ast.rs](../impls/rs-canonicalizer/src/ast.rs)

## Context

[phase-0-plan.md § Stage 3](../plans/phase-0-plan.md) lists "formal grammar for Tacit-Lite AST as a Rust enum hierarchy" as a deliverable. A Rust enum matching [canonical-text-format.md § 2](../plans/canonical-text-format.md) already exists inside the Rust canonicalizer at [impls/rs-canonicalizer/src/ast.rs](../impls/rs-canonicalizer/src/ast.rs). It has one variant per canonical tag — `Lam`, `App`, `Let`, `Rec`, `Module`, `If`, `Match`, `Arm`, `Record`, `Proj`, `Ctor`, `Ann`, `Var`, `Int`, `Str`, `Sym`, `Hole`, `PatWild`, `PatVar`, `PatCtor` — and the variant fields carry the child types from the spec's § 2 arity table (e.g. `Lam { body: Box<Node> }`, `App { fn_: Box<Node>, arg: Box<Node> }`, and so on). The source file is ~90 lines after `cargo fmt` expansion; readers wanting the exact variant shapes should click through to the file rather than rely on a snippet embedded here.

The enum was written against [canonical-text-format.md § 2](../plans/canonical-text-format.md), not the other way around, so it is a *conforming transcription* of the frozen spec — not a parallel authority. [ADR 0013](0013-canonical-text-format-frozen.md)'s freeze covers the underlying grammar; this ADR concerns only the *packaging* of the enum that encodes it.

### Design space

- **(A) Leave in canonicalizer.** Today's state. The enum is an implementation detail of `rs-canonicalizer`; other future Rust code (Phase 1 parser, Phase 1 LLVM emitter, Phase 4 debugger) would re-derive it from the spec.
- **(B) Promote to a shared crate `crates/tacit-ast`.** Stand up a Cargo workspace with one crate containing the enum + documentation, depended on by `rs-canonicalizer` and whatever Phase 1+ crates want it.
- **(C) Duplicate-and-maintain.** Each Rust consumer keeps its own copy, kept in sync manually. Explicitly not considered — this is what content-addressing exists to avoid.

### Constraint from CLAUDE.md

> **No Phase 1 work.** Don't write a parser, AST walker, or LLVM emitter until Phase 0's exit criteria are met. Rust AST enum definitions that derive from the spec are in scope; anything that consumes or produces them is not.

And:

> Don't add compiler scaffolding "to save time later" — Phase 1 will do it with the benefit of a frozen spec.

This points squarely at (A). A shared crate would be scaffolding for consumers that don't yet exist.

## Decision

**The Rust AST enum lives in the canonicalizer at [impls/rs-canonicalizer/src/ast.rs](../impls/rs-canonicalizer/src/ast.rs) and is not promoted to a shared crate during Phase 0.** It is marked as a *conforming reference* — a Rust transcription of [canonical-text-format.md § 2](../plans/canonical-text-format.md), kept in sync with the spec, authoritative only insofar as it matches the spec.

Concretely:

1. **No Cargo workspace or `crates/tacit-ast` in Phase 0.** Defers to Phase 1, which will spin up the compiler workspace and is the natural moment to decide which crates the AST belongs in (likely a `tacit-core` or `tacit-ast` alongside the parser and type checker).
2. **The module docstring in `ast.rs` already credits the spec** as the source of truth ("Kinds and arities match canonical-text-format.md § 2"). This is the Stage 3 "deriving from the canonical spec" deliverable: the enum is in a known, citable location and its relationship to the spec is documented at the type level.
3. **Any Phase 1 consumer that needs the enum** copies it into the new compiler crate (or depends on whatever crate is created there). At that point, a short ADR will decide whether to keep the canonicalizer's copy or retire it in favor of a workspace dependency.
4. **The Python canonicalizer's AST** ([impls/py-canonicalizer/](../impls/py-canonicalizer/)) remains a parallel conforming reference for Python consumers on the same footing. Neither impl's AST is authoritative over the spec.

## Alternatives considered

- **Promote to `crates/tacit-ast` now.** Rejected. CLAUDE.md's "no scaffolding to save time later" rule applies directly: there are no consumers in Phase 0 beyond the canonicalizer itself. A shared crate would add a Cargo workspace (Stage 5 work), a publish/versioning story, and a second maintenance surface for zero present benefit. Phase 1 can perform the promotion with full knowledge of what the parser and emitter want to see.
- **Copy the enum into `docs/` or `plans/` as a code sample.** Rejected. A code sample in a spec doc inevitably drifts from the real enum. The canonicalizer's `ast.rs` is the live copy, already referenced from this ADR; readers who want the Rust enum click through to the impl.
- **Generate the enum from the spec mechanically.** Rejected as over-engineering. The spec's § 2 table is 20 rows and the enum is 20 variants — a one-time hand transcription that is trivially reviewed against the spec. A code-generator adds a build-time dependency and a second format (the spec's machine-readable form) for no payoff at this size.
- **Switch to trait-object / dyn-typed AST.** Rejected. Rust's `enum` + pattern matching is the ergonomic choice for a fixed, closed set of kinds, which is what canonical form commits to. Opening the AST to extension via traits would invite implicit-extension kinds that canonical text can't represent.

## Consequences

- **Stage 3's "Rust AST enum" deliverable is satisfied by the existing `ast.rs` file.** No new code, no new crate. The spec + the enum + this ADR together document Stage 3's intent.
- **Phase 1 bears the promotion cost.** When the workspace is created, the enum moves (or is re-homed alongside a parser crate). Low cost — the enum is small, stable, and frozen against the spec.
- **The Python canonicalizer's ast.py has equal standing.** Any future impl (Go, OCaml, TypeScript) adds another conforming transcription. None is authoritative over the others or over the spec.
- **If a consumer is added during Phase 0 and needs the enum** (unlikely — Phase 0 is spec-only), this ADR is revisited and the crate-promotion happens early.

## Related decisions

- [ADR 0013](0013-canonical-text-format-frozen.md) — the canonical text format freeze; the enum transcribes its § 2 table.
- [CLAUDE.md § Ground rules](../CLAUDE.md) — "No Phase 1 work" and "Don't add compiler scaffolding to save time later" — the two rules this ADR follows.
- [phase-0-plan.md § Stage 3](../plans/phase-0-plan.md) — the deliverable this ADR closes.
- [phase-0-plan.md § Stage 5](../plans/phase-0-plan.md) — Cargo workspace scaffolding, where any workspace promotion would naturally land.
