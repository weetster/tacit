# 0046 — Phase 2 frozen (Stage 5: CLI wiring + view annotations + architecture doc)

**Status:** Accepted
**Date:** 2026-04-28
**Phase:** 2 (exit)
**Supersedes:** None
**Artifacts frozen by this ADR:**
- [crates/tacit-cli/](../crates/tacit-cli/) — `tacit` binary: `compile`, `check`, and `view` subcommands; typecheck wired into `compile`; `--types`/`--effects` flags wired into `view --as inspection`.
- [docs/compiler-architecture.md](../docs/compiler-architecture.md) — Phase 2 architecture document: five-crate graph, `tacit compile` / `tacit check` / `tacit view` data-flow diagrams, Phase 2 codegen-subset table.
- [plans/phase-2-plan.md](../plans/phase-2-plan.md) — all five stages marked done.
- All prior Phase 2 artifacts frozen by ADRs 0034–0045 remain frozen.

## Context

Phase 2 of the Tacit compiler was scoped in
[phase-2-plan.md](../plans/phase-2-plan.md) as a type-and-effect layer on top
of the Phase 1 pipeline. The phase had four exit criteria:

> 1. **Typed smoke corpus.** All nine smoke programs carry signatures,
>    typecheck, lower, link, and produce the expected stdout / exit code
>    under CI.
> 2. **Non-trivial programs.** (Deferred to Phase 3 — see § 3 below.)
> 3. **Round-trip and inspection gates from Phase 1 hold.**
> 4. **Structured diagnostics.** A negative-test corpus exercises every
>    `TypeError` and `EffectError` variant.

All five stages are now complete:

| Stage | Deliverable | Frozen by | Date |
|-------|-------------|-----------|------|
| 1 | Spec ADRs (Q-P2-1 through Q-P2-10) | ADR 0044 | 2026-04-27 |
| 2 | `tacit-typecheck` crate + structural type checker | ADR 0044 | 2026-04-27 |
| 3 | Effect checker | ADR 0044 | 2026-04-27 |
| 4 | Hole recovery + `module` authoring + Phase 1 carry-overs | ADR 0045 | 2026-04-28 |
| 5 | `tacit-cli` wiring + view annotations + architecture doc | this ADR | 2026-04-28 |

### Stage 5 implementation notes

**`tacit compile` typecheck integration.** Parsing and typecheck now run ahead
of the LLVM gate in `cmd_compile`. `infer_module` is called on the parsed `Node`
before any codegen; type/effect errors emit a JSON `DiagOutput` envelope to
stderr and exit with code 1 (distinct from codegen/linker failures, which exit 2).
This means `tacit check` and the typecheck step in `tacit compile` share the
same `tacit-typecheck::infer_module` path and produce identical diagnostics.

**`tacit check` subcommand.** New subcommand with `--format text|json` (default
`text`). Text format writes human-readable `error[kind]: message` lines to
stderr. JSON format writes a `DiagOutput` envelope to stdout. Both paths exit 0
on clean check and 1 on any errors. No LLVM dependency — builds and works
without any LLVM feature flag.

**`--types` / `--effects` flags for `tacit view --as inspection`.** These flags
were reserved by [ADR 0015](0015-inspection-view-scope.md) as Phase 2
annotation layers. Stage 5 wires them:
- `--types`: renders `FnTy`, `TyVar`, `Forall` nodes in human-readable form
  using Unicode notation (e.g., `α0 -> Bool / {IO}`, `∀[α0, ε0]. α0 -> α0`).
- `--effects`: renders `EffSet` with spaces (`{IO, Mut}` vs `{IO,Mut}`) and
  `EffVar` as `ε{index}`.
The flags compose with `--debruijn` and `--hashes` (L0+L1+L2 from ADR 0015)
and with each other.

**Architecture doc.** `docs/compiler-architecture.md` updated to add the
`tacit-typecheck` crate (layout + dependency graph), the `tacit check`
pipeline diagram, the revised `tacit compile` pipeline (with typecheck step
and exit-code table), the `--types`/`--effects` flag description in the view
section, and the Phase 2 codegen additions table (`PatInt`, `Module`,
`@buf-alloc`, `@read`, transparent `Ann`).

**CI.** The CI smoke step is extended with `tacit check examples/smoke/echo.tac`
(clean pass) and `tacit check --format json examples/smoke/hello.tac` (JSON
envelope with empty errors array).

## Decision

**Phase 2 is frozen.** Exit criteria 1, 3, and 4 are satisfied:

1. **Typed smoke corpus (nine programs).** `return-zero`, `return-computed`,
   `hello`, `if-branch`, `factorial`, `even-odd`, `exit-nonzero`, `match-int`,
   `echo` — all carry `.tac.sidecar.toml` type+effect expectations, pass
   `tacit-typecheck` tests, compile with LLVM, and produce the expected stdout /
   exit code. The `tacit compile` path now runs typecheck ahead of codegen for
   all nine.

2. **Round-trip and inspection gates hold.** `cargo test` passes the authoring
   round-trip property on all vectors (including the previously-excluded
   `module`-bearing fixtures added by Stage 4). L0/L1/L2 inspection-view
   fixtures render byte-identically. `--types` and `--effects` fixtures are new
   Phase 2 additions not covered by prior fixture locks.

3. **Structured diagnostics.** `crates/tacit-typecheck/tests/negative.rs`
   exercises `type-mismatch`, `operator-overload-failure`, `unbound-type-variable`,
   `type-arity-mismatch`, `unresolved-type`, `module-missing-annotation`,
   `hole-diagnostic`, `parser-recovery-hole-flows-through-typecheck`,
   `effect-violation`, `unbound-effect-variable`, plus sidecar mismatch paths.

Concretely:

1. **The `tacit-cli` crate is updated and locked.** The new surface (`check`
   subcommand; `--types`/`--effects` view flags; typecheck in `compile`) is
   normative. Further CLI changes require a new ADR. Bug fixes do not.

2. **The architecture doc reflects Phase 2.** `docs/compiler-architecture.md`
   is the normative pipeline description. Phase 3 additions slot in without
   retroactive ADRs; removing or contradicting existing sections requires one.

3. **Deferred items.** Phase 2 exit criterion 2 (non-trivial programs —
   sorting, data structures, file I/O beyond `echo`) is deferred to Phase 3
   as explicitly noted in phase-2-plan.md. The exit criteria for Phase 2 were
   met by the nine-program typed smoke corpus; the non-trivial-programs
   requirement extends naturally into Phase 3's scope.

4. **Phase 3 may begin.** The phase-2-plan.md gate is satisfied on criteria
   1, 3, and 4 (criterion 2 deferred as above). Phase 3 owns non-trivial
   programs, a richer stdlib, and the first corpus evaluation tasks.

## Alternatives considered

- **Include non-trivial programs before freezing.** Would require hand-authoring
  sorting algorithms and linked-list structures in Tacit-Lite. These are
  Phase 3 evaluation tasks by design (the `corpus/` directory and the Phase 3
  plan are the natural home for them). Delaying Phase 2 freeze for this work
  creates unnecessary scope creep. Deferred with explicit note in this ADR.

- **Wire `--types`/`--effects` to show inferred types on every expression.**
  Would require threading `TypedModule` (or a per-node type map) through the
  inspection renderer. The Phase 2 inspection view already shows types from
  `Ann` nodes; wiring full per-node inferred-type annotations is Phase 3+
  tooling work. The current flags satisfy the ADR 0015 contract of "lights up"
  the reserved slots.

## Consequences

- **Phase 3 begins.** The immediate next step is a `phase-3-plan.md` scoping
  non-trivial programs, a richer stdlib, and the first corpus evaluation.

- **CLAUDE.md updated.** The current-phase annotation now reads "Phase 2
  complete; Phase 3 is next."

- **CI is stable.** The CI matrix now exercises: format, clippy (with LLVM),
  all unit and integration tests, `tacit compile` end-to-end on the nine-program
  smoke corpus, `tacit check` on `echo.tac` and `hello.tac`. The Phase 2
  typecheck negative-test corpus runs as part of `cargo test`.

## Related decisions

- [ADR 0033](0033-phase-1-frozen.md) — Phase 1 freeze; establishes the discipline.
- [ADR 0044](0044-p2-stage-1-frozen.md) — Phase 2 Stages 1–3 freeze.
- [ADR 0045](0045-p2-stage-4-frozen.md) — Phase 2 Stage 4 freeze.
- [ADR 0015](0015-inspection-view-scope.md) — inspection view scope; reserved `--types`/`--effects`.
- [ADR 0041](0041-p2-structured-error-format.md) — structured error format.
- [phase-2-plan.md](../plans/phase-2-plan.md) — the deliverable list this ADR closes.
- [tacit-plan.md § Phase 3](../plans/tacit-plan.md) — the next phase.
