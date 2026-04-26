# 0032 — Phase 1 Stage 4 frozen

**Status:** Accepted
**Date:** 2026-04-25
**Phase:** 1, Stage 4 (exit)
**Supersedes:** None
**Artifacts frozen by this ADR:**
- [crates/tacit-codegen/](../crates/tacit-codegen/) — analysis + emission layers as of this commit.
- [examples/smoke/](../examples/smoke/) — the seven-program Phase 1 smoke corpus.
- [docs/compiler-architecture.md](../docs/compiler-architecture.md) — the LLVM-version pin and dev-loop install commands.
- `.github/workflows/ci.yml` — the `llvm-19-dev` install + `cargo test --features tacit-codegen/llvm19-1` invocation.

## Context

Stage 4 of [phase-1-plan.md](../plans/phase-1-plan.md) builds the AST → LLVM IR → object → executable pipeline. The structural decisions were already in place: ADR 0024 picked `inkwell`, ADRs 0025–0028 fixed the libc surface and call shape, ADR 0030 added arith/cmp intrinsics, ADR 0031 scoped the distribution model. What remained for the exit was:

1. Picking a concrete LLVM version (deferred by ADR 0024 to "the Stage 4 familiarization spike").
2. Demonstrating end-to-end execution on every Phase 1 smoke program.
3. Wiring CI so the exit gate actually runs.

Items 2 and 3 were blocked on item 1 — the smoke tests are gated behind a `llvm<N>-<M>` feature flag and there was no LLVM library reachable from the build machine until a version was chosen.

The pin landed in commit 6564a20 (LLVM 19 / `inkwell` 0.9 with the `llvm19-1` feature). Running the smoke gate locally surfaced three small follow-on issues that are recorded here so the freeze trail is honest:

1. **`inkwell` 0.9 API drift.** `CallSiteValue::try_as_basic_value()` returns `inkwell::values::ValueKind` (a typed enum), not `Either`. The Stage 4 emitter was written against the older `.left()` shape and had to switch to `.basic()`.

2. **`lookup_var` lifetime over-tightening.** The original signature `fn lookup_var<'ctx>(env: &'ctx [Binding<'ctx>], …) -> Result<&'ctx Binding<'ctx>>` tied the env borrow lifetime to the LLVM context lifetime, making the borrow checker unhappy at every read site. Relaxed to independent lifetimes `<'a, 'ctx>` so the env is borrowed for its natural scope.

3. **Hello-world exit code.** Appendix A.5 of phase-1-plan.md showed `ret i32 0` for `hello.tac`, but the source listing in A.1 (`@write 1 "hello, world\n" 13`) evaluates to `13` (write's byte-count return), and `compile_program` uses the program's value as the exit code. The smoke test asserts exit `0`. Resolved by changing the source to `let _ = @write 1 "hello, world\n" 13 in 0`, which matches the IR appendix A.5 already specified. Appendix A.1 / A.3 / A.4 were updated to the let-discard form.

None of the three required spec changes — they are emitter and corpus-source corrections. The Phase 1 ADRs (0023–0031) and the Phase 0 frozen artifacts are unaffected.

`libpolly-19-dev` was added alongside `llvm-19-dev` in CI: `llvm-sys` 191 links `Polly` statically and the Debian split puts that in a separate package.

## Decision

**Stage 4 is frozen.** The Phase 1 codegen subset listed in `docs/compiler-architecture.md § Phase 1 codegen subset` is the implemented surface. The seven-program smoke corpus is the regression contract.

Concretely:

1. **LLVM version is pinned to 19** (`inkwell` 0.9 with the `llvm19-1` feature). Bumping LLVM is a deliberate release-engineering task per [ADR 0031 § 2](0031-llvm-distribution-and-self-hosting.md): touch `Cargo.toml`, `docs/compiler-architecture.md`, `.github/workflows/ci.yml`, re-run the smoke corpus on every supported platform, write a follow-up ADR.

2. **The seven programs in `examples/smoke/` are the Stage 4 exit gate.** `return-zero`, `return-computed`, `hello`, `if-branch`, `factorial`, `even-odd`, `exit-nonzero` all canonicalize, parse, lower, link, and run with the expected stdout / exit code under `cargo test -p tacit-codegen --features llvm19-1`. The CI job runs the same invocation on `ubuntu-latest`.

3. **Smoke #7 (`match-int.tac`) and smoke #8 (`echo.tac`) are deferred.** Per the phase-1-plan.md Stage 4 progress notes, both are gated on follow-up ADRs (`pat-int` canonical extension and the writable-buffer model). They are out of the Stage 4 exit gate but remain in the corpus inventory in [Appendix B](../plans/phase-1-plan.md) for the Phase 2 work.

4. **The `tacit-codegen` analysis + emission split is locked.** Analysis modules (`analysis`, `error`, `primitives`) build without LLVM; the `compile` module is gated behind any per-version `llvm<N>-<M>` feature. New codegen work must preserve this split so the analysis layer stays testable on machines without LLVM.

5. **Changes to the codegen surface or the smoke corpus after this freeze require a new ADR**, identical to the Stage 2/3 freeze discipline imposed by ADRs 0013 and 0017. Bug fixes to existing behavior (an emitter producing the wrong IR for an in-scope program) are not changes for this purpose; introducing a new AST kind, a new primitive, or a new smoke program is.

## Alternatives considered

- **Defer freeze until smoke #7 + #8 land.** Would require resolving the `pat-int` canonical extension and the writable-buffer model under Phase 1, contradicting the Phase 0 freeze ([ADR 0013](0013-canonical-text-format-frozen.md)) and re-opening canonical-text decisions that are explicitly closed. Rejected: the deferred ADRs are correctly the Phase 2 owner, and a partial Stage 4 with a recorded deferral list is healthier than holding Stage 4 open for cross-phase work.

- **Special-case `main` to always return 0.** Would let `hello.tac` keep its original `@write 1 "hello, world\n" 13` shape with the exit code clamped. Rejected: it breaks `return-zero`, `return-computed`, `factorial`, `even-odd`, and `exit-nonzero`, all of which rely on the program's value as the exit code. The let-discard form is what users will write anyway when they want a side-effect-only `main`.

- **Pin LLVM 18 instead of 19.** Available on more aged platforms but missing from Debian bookworm's default apt set and trailing `inkwell`'s feature support by one minor version. Rejected: ADR 0031's "newest pre-built that's available everywhere we ship CI" rule pointed at 19, and the cost of re-bumping later is the same as the cost of pinning later. LLVM 19 also matches what `clang-19` ships in the same apt repo, which simplifies the linker step.

- **Skip the freeze ADR and let Stage 5 absorb the LLVM pin.** Stage 5 is the CLI + architecture-doc step, not the codegen step. Folding the codegen freeze into Stage 5's documentation work would smear two unrelated exit gates and make it unclear which one a future regression is anchored against. Rejected.

## Consequences

- **Phase 1 has one stage left.** Stage 5 (CLI + architecture doc) is the only remaining gate; Stages 1–4 are all frozen.

- **CI now actually exercises the codegen path.** Prior CI installed `llvm-19-dev` but ran `cargo test` without the feature flag, so `tests/smoke.rs` (`#![cfg(feature = "llvm")]`) was a no-op. The CI job now runs the codegen tests with `--features tacit-codegen/llvm19-1`; the `clippy` step also gets the feature so the `compile` module is type-checked.

- **A toolchain bump from before this freeze surfaces a fmt drift.** `rustfmt 1.8.0-stable` (Rust 1.91) reformats files that were committed against an earlier version. The drift was applied in this freeze commit so `cargo fmt --check` is green.

- **Smoke #7 and #8 stay in the inventory but not in CI.** When the Phase 2 ADRs that unblock them land, they re-enter the corpus and the CI gate without an ADR amendment here — the freeze locks the implemented set, not the inventory.

- **`docs/compiler-architecture.md` is no longer "TODO until LLVM is reachable."** The file's lead paragraph was written when the version was unpinned; Stage 5's architecture-doc deliverable will rewrite it more thoroughly, but the LLVM-pin section is now load-bearing reference material rather than a placeholder.

- **`inkwell` API churn is a real risk for future LLVM bumps.** The `try_as_basic_value()` rename alone shows that minor `inkwell` releases between LLVM majors can break the codegen. Future bumps must rebuild and run the smoke corpus before merging the pin change, not after.

## Related decisions

- [ADR 0013](0013-canonical-text-format-frozen.md) — Phase 0 Stage 2 freeze; sets the discipline this ADR follows.
- [ADR 0017](0017-stage-3-frozen.md) — Phase 0 Stage 3 freeze; structural template.
- [ADR 0018](0018-stage-5-frozen.md) — Phase 0 Stage 5 freeze; the prior repo-scaffolding freeze whose CI shape this stage extends.
- [ADR 0024](0024-llvm-bindings-inkwell.md) — `inkwell` choice; this ADR closes its deferred "exact version pair chosen at Stage 4" clause.
- [ADR 0025](0025-phase-1-libc-surface.md) — libc surface; implemented as the `LIBC` arm of `PrimKind`.
- [ADR 0026](0026-phase-1-closed-lambdas.md) — closed lambdas; implemented in `Compiler::hoist_lambda` + `compile_app`.
- [ADR 0027](0027-phase-1-rec-lowering.md) — rec lowering; implemented in `Compiler::compile_rec`.
- [ADR 0028](0028-phase-1-libc-call-surface.md) — `@name` surface; implemented as the `Sym` head dispatch in `compile_app`.
- [ADR 0030](0030-phase-1-arith-primitives.md) — arith/cmp intrinsics; implemented in `emit_arith` / `emit_cmp`.
- [ADR 0031](0031-llvm-distribution-and-self-hosting.md) — distribution + self-hosting; this ADR consumes its "pick the newest pre-built apt version" guidance for the LLVM 19 pin.
- [phase-1-plan.md § Stage 4](../plans/phase-1-plan.md) — the deliverable list this ADR closes. Stage 4 status updates to **Frozen** concurrently with this ADR.
