# 0033 — Phase 1 frozen

**Status:** Accepted
**Date:** 2026-04-26
**Phase:** 1 (exit)
**Supersedes:** None
**Artifacts frozen by this ADR:**
- [crates/tacit-cli/](../crates/tacit-cli/) — `tacit` binary: `compile` and `view` subcommands.
- [docs/compiler-architecture.md](../docs/compiler-architecture.md) — full Stage 5 architecture document (crate graph, pipeline diagrams, sidecar flow).
- [plans/phase-1-plan.md](../plans/phase-1-plan.md) — all five stages marked done.
- [CLAUDE.md](../CLAUDE.md) — Phase 1 complete; Phase 2 is the next phase.
- All prior Phase 1 artifacts frozen by ADRs 0029–0032 remain frozen.

## Context

Phase 1 of the Tacit compiler was scoped in
[phase-1-plan.md](../plans/phase-1-plan.md) as a minimum viable pipeline:
authoring `.tac` source → canonical AST → LLVM IR → native executable, plus
a lossless authoring-view round-trip and an inspection-view renderer, all
behind a `tacit` CLI. The phase had three exit criteria:

> 1. From a `.tac` file written in the authoring view, the user can
>    canonicalize it, compile it, run it, and observe expected output.
>    End-to-end CI run demonstrates this on every smoke program.
> 2. `tacit view` round-trips authoring ↔ canonical (via sidecar) on every
>    Phase 0 test vector, and renders inspection-view L0/L1/L2 fixtures
>    byte-identically.
> 3. The compiler architecture doc is sufficient that another engineer could
>    reproduce the crate layout and pipeline ordering without reading source.

All five stages are now complete:

| Stage | Deliverable | Frozen by | Date |
|-------|-------------|-----------|------|
| 1 | Cargo workspace + AST crate promotion | ADR 0029 | 2026-04-24 |
| 2 | Authoring view + sidecar round-trip | ADR 0029 | 2026-04-24 |
| 3 | Inspection-view renderer | — (in-scope with Stage 2) | 2026-04-25 |
| 4 | LLVM IR emitter + smoke corpus | ADR 0032 | 2026-04-25 |
| 5 | `tacit` CLI + architecture doc | this ADR | 2026-04-26 |

### Stage 5 implementation notes

The CLI crate (`tacit-cli`) was added as the fourth workspace member. Its
feature flags mirror `tacit-codegen` — building without an LLVM feature
produces a binary that supports `tacit view` but gives an actionable error
for `tacit compile`. This avoids making `view` depend on a system LLVM
installation.

`compile_to_ir_string` was added alongside the existing `compile_to_object`
in `tacit-codegen::compile`, giving the `--emit-llvm-ir` flag a clean
internal entry point without requiring `inkwell` as a direct dependency of
`tacit-cli`.

The CI job was extended with two new steps:

1. `cargo build --features tacit-cli/llvm19-1 --bin tacit` — verifies the
   binary builds with LLVM on every push.
2. A bash smoke step that runs `tacit compile examples/smoke/hello.tac -o
   /tmp/tacit-hello`, executes the result, and asserts the output.

The `--as` CLI argument for `tacit view` is a string-literal argument name
in clap (`#[arg(long = "as")]`); `as` is a Rust keyword but is legal in
attribute string positions. The struct field is named `view_format`.

## Decision

**Phase 1 is frozen.** All three exit criteria are satisfied:

1. **End-to-end CI on all smoke programs.** `cargo test --features
   tacit-codegen/llvm19-1` runs all seven smoke programs (`return-zero`,
   `return-computed`, `hello`, `if-branch`, `factorial`, `even-odd`,
   `exit-nonzero`) as part of `tacit-codegen`'s test suite. The CLI smoke
   step in CI additionally verifies `hello` through the `tacit compile`
   binary. All programs canonicalize, parse, lower, link, and produce the
   expected stdout / exit code.

2. **Round-trip and inspection-fixture gates.** `cargo test` passes the full
   authoring-view round-trip property on every in-scope Phase 0 test vector
   (the skip-list in `crates/tacit-views/tests/round_trip.rs` enumerates
   the spec-excluded cases). Inspection-view L0/L1/L2 fixtures render
   byte-identically.

3. **Architecture doc.** `docs/compiler-architecture.md` now contains the
   four-crate dependency graph, the `tacit compile` and `tacit view`
   data-flow diagrams, the sidecar-flow explanation, and the LLVM feature
   flag table. The Phase 1 codegen subset table remains as a reference for
   Phase 2.

Concretely:

1. **The `tacit-cli` crate is locked.** Changes to the CLI surface (new
   subcommands, new flags, changed argument names, changed exit-code
   semantics) require a new ADR. Bug fixes to existing behavior do not.

2. **The architecture doc is load-bearing.** `docs/compiler-architecture.md`
   is the normative description of the pipeline. Additions that reflect
   Phase 2 work (type-checker crate, effect-annotation passes) can be made
   without an ADR; removing or contradicting existing sections requires one.

3. **Deferred items from Phase 1 remain deferred.** Per phase-1-plan.md
   Stage 2 exclusions and ADR 0032 § 3:
   - Smoke #7 (`match-int.tac`) — blocked on a `pat-int` canonical
     extension ADR.
   - Smoke #8 (`echo.tac`) — blocked on the writable-buffer model ADR.
   - Top-level `module` syntax in the authoring view — deferred to Phase 2.
   - Hole-node recovery (ADR 0023) — deferred to Phase 2.
   These items are Phase 2 scope; they do not block Phase 2 from starting.

4. **Phase 2 may begin.** The phase-1-plan.md gate ("Phase 2 must not begin
   until all three criteria are met") is satisfied. Phase 2 owns the type
   and effect system; its first act should be a `phase-2-plan.md` that
   scopes the work against the Phase 1 baseline frozen here.

## Alternatives considered

- **No Phase 1 freeze ADR; just update CLAUDE.md.** Inconsistent with the
  precedent set by ADRs 0013, 0017, 0018, and 0032. Each prior phase
  boundary got an explicit ADR that records what was built, what was
  deferred, and what the constraints on the next phase are. Omitting it
  would leave the phase boundary implicit and harder to audit. Rejected.

- **Include smoke #7 and #8 before freezing.** Would require resolving the
  `pat-int` canonical extension and the writable-buffer model, both of
  which are cross-phase concerns anchored in Phase 0's frozen canonical
  format. ADR 0032 already deferred them with explicit rationale. Rejected.

## Consequences

- **Phase 2 begins.** The immediate next step is a `phase-2-plan.md`
  document scoping the type and effect checker. The Phase 1 crate graph
  (`tacit-canonical` → `tacit-views` → `tacit-cli`, `tacit-codegen` →
  `tacit-canonical`) is the starting baseline; Phase 2 will introduce a
  `tacit-typecheck` crate and wire it into `tacit-cli`.

- **CLAUDE.md updated.** The current-phase annotation in CLAUDE.md now
  reads "Phase 1 complete; Phase 2 is next." Contributors starting work
  from a fresh checkout see the correct state.

- **CI is stable.** The CI matrix now exercises the full Phase 1 surface:
  format, clippy (with LLVM), codegen smoke tests, and the CLI smoke step.
  Phase 2 will add typecheck tests and likely a new CI step, but the
  existing steps remain as the Phase 1 regression contract.

- **`stdlib/libc-effects.toml` is dormant but load-bearing.** It exists for
  Phase 2's effect checker to consume. It is not changed by this freeze but
  is explicitly in scope for Phase 2's first sprint.

## Related decisions

- [ADR 0013](0013-canonical-text-format-frozen.md) — Phase 0 freeze; sets the discipline.
- [ADR 0017](0017-stage-3-frozen.md) — Phase 0 Stage 3 freeze.
- [ADR 0018](0018-stage-5-frozen.md) — Phase 0 Stage 5 freeze.
- [ADR 0029](0029-cargo-workspace-layout.md) — Stage 1 workspace promotion.
- [ADR 0032](0032-stage-4-frozen.md) — Stage 4 freeze; this ADR extends it.
- [phase-1-plan.md](../plans/phase-1-plan.md) — the deliverable list this ADR closes.
- [tacit-plan.md § Phase 2](../plans/tacit-plan.md) — the next phase.
