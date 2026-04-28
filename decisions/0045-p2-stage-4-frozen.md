# 0045 — Phase 2 Stage 4 frozen

**Status:** Accepted
**Date:** 2026-04-28
**Phase:** 2, Stage 4 (exit)
**Closes:** [phase-2-plan.md § Stage 4](../plans/phase-2-plan.md)
**Depends on:** [ADR 0044](0044-p2-stage-1-frozen.md) (Stage 1 spec surface); [ADR 0037](0037-p2-pat-int.md), [ADR 0038](0038-p2-writable-buffer.md), [ADR 0039](0039-p2-module-authoring-syntax.md), [ADR 0040](0040-p2-hole-recovery.md) (the four Stage 4 spec ADRs).

## Context

Stage 4 of [phase-2-plan.md](../plans/phase-2-plan.md) closed the four Phase 1 carry-overs that [ADR 0033](0033-phase-1-frozen.md) § 3 deferred into Phase 2:

1. **Smoke #7 — `match-int.tac`** — integer literal pattern matching via `pat-int`.
2. **Smoke #8 — `echo.tac`** — writable stack buffer (`@buf-alloc`) + `@read`/`@write` round-trip.
3. **Top-level `module` authoring syntax** — `module { name = expr ; ... }` at top level.
4. **Hole-node parser recovery** — `ParseError` replaced by structured `Hole` nodes with diag-ids.

All four shipped together on 2026-04-28.

## What was built

### `tacit-views` changes

**Lexer (`crates/tacit-views/src/authoring/lex.rs`):**
- `Token::Module` added; `"module"` dispatches to it in `keyword_or_ident`.
- The `@` arm now consumes the following identifier using `lex_sym_name`, which allows `-` in continuations (`buf-alloc`, etc.). Regular identifiers still forbid hyphens.
- `is_sym_name_cont` and `lex_sym_name` helpers added.

**Parser (`crates/tacit-views/src/authoring/parse.rs`):**
- `HoleDiag { diag_id, message }` struct; `holes: Vec<HoleDiag>` on `Parser`.
- `advance_to_sync` — skips to next `;`, `}`, or EOF at depth 0.
- `recover_expr` — emits a `Hole` node, pushes to `holes`, calls `advance_to_sync`. Used in all expression-level error recovery sites.
- `parse_module` — two-pass parse (collect names, restore position, parse bodies with all names in scope). Identical structure to `parse_rec`; no trailing `in body` clause.
- `parse_authoring` dispatches to `parse_module` when the first token is `Token::Module`.
- `Token::Module` in expression position recovers with a `Hole` (diag-id: `"module-binding-error"`).
- `parse_pattern` and `parse_pattern_atom` handle `Token::Int(s)` → `Node::PatInt { value }` (ADR 0037); unknown pattern tokens recover with a `Hole`.
- `parse_head_atom` and `parse_pattern_atom` error arms use `recover_expr` instead of hard-failing.

**Round-trip tests (`crates/tacit-views/tests/round_trip.rs`):**
- `"28-module-one-binding.canonical"`, `"32-pat-int-match.canonical"`, `"33-buf-alloc-read.canonical"` removed from the SKIP list and now pass.
- New `parse_*` tests: `parse_module_one_binding`, `parse_module_two_bindings`, `parse_pat_int_match`, `parse_buf_alloc_sym`, `parse_hole_recovery_unknown_token`.

### `tacit-typecheck` changes

**`ty.rs`:** `Ty::Buf` added — leaf type for stack-allocated byte buffers. `Display`, `is_ground`, and `unify` updated to cover it.

**`primitives.rs`:**
- `"write"` — buffer arg changed from `Ty::Str` to `Ty::Unknown` so both string literals and `Buf` values unify.
- `"read"` — same `Ty::Unknown` buffer arg; effect changed from `{IO}` to `{IO, Mut}` per ADR 0038.
- `"buf-alloc"` added: `Int → Buf / {Alloc}`.
- `fn1_alloc`, `fn3_mut_io` helpers added.
- `is_alloc_prim` predicate added.

**`error.rs`:** `ty_to_json` handles `Ty::Buf → {"sym": "Buf"}`.

**Smoke sidecar files:**
- `examples/smoke/match-int.tac.sidecar.toml` — `type = "Int"`, `effects = []`.
- `examples/smoke/echo.tac.sidecar.toml` — `type = "Int"`, `effects = ["Alloc", "IO", "Mut"]`.

**Tests (`crates/tacit-typecheck/tests/`):**
- `smoke.rs` — `smoke_match_int`, `smoke_echo` added.
- `negative.rs` — `neg_parser_recovery_hole_flows_through_typecheck`: verifies that `lambda x. => x` (FatArrow in expression position) parses without error, yields a `Hole` in the AST, and produces a hole diagnostic through `infer_module`.

### `tacit-codegen` changes

**`primitives.rs`:** `PrimKind::BufAlloc` added (arity 1); `"buf-alloc"` maps to it.

**`compile.rs`:**
- `Binding::Ptr(PointerValue<'ctx>)` — new variant for stack-buffer handles; not a first-class integer value.
- `compile_let` — detects `App { fn_: Sym("buf-alloc"), arg: Int(N) }` pattern and emits `alloca [N x i8]` + `gep` to produce `Binding::Ptr`. All other `let` RHS shapes fall through unchanged.
- `compile_expr` (`Var` arm) — `Binding::Ptr` in integer-value position returns `Unsupported`.
- `compile_app` (`Var` arm) — `Binding::Ptr` in call-head position returns `AppNonFunction`.
- `compile_primitive_call` (`BufAlloc` arm) — returns `Unsupported` (valid `@buf-alloc` must appear as a `let` RHS, not free in an expression).
- `compile_buffer_arg` — accepts `Node::Var` resolving to `Binding::Ptr` (the `@buf-alloc` path) in addition to `Node::Str` (the string-literal path). Removed the old `int_to_ptr` fallback.
- `compile_match` — `Node::PatInt { value }` arm added: emits `icmp eq` + conditional branch, identical shape to the `PatCtor`-with-numeric-name arm it supplements.

**Smoke files:**
- `examples/smoke/match-int.tac` — `match 0 with | 0 => 42 | _ => 0`.
- `examples/smoke/echo.tac` — `let buf = @buf-alloc 1024 in let n = @read 0 buf 1024 in let _ = @write 1 buf n in 0`.

**Tests (`crates/tacit-codegen/tests/smoke.rs`):**
- `match_int` — exit code 42, no stdout.
- `echo` — stdin `"hi\n"` → stdout `"hi\n"`, exit code 0.
- `run_with_stdin` helper added for piped-stdin tests.

## Decision

**Stage 4 is frozen.** The four Phase 1 carry-overs are closed as concrete features. The authoring view, typecheck crate, and codegen crate are the authoritative production implementations; test output (exit codes, stdout, round-trip bytes) is the ground truth.

Concretely:

1. **The nine-program smoke corpus is the Phase 2 regression baseline.** Phase 1's seven (`return-zero`, `return-computed`, `hello`, `if-branch`, `factorial`, `even-odd`, `exit-nonzero`) plus Stage 4's two (`match-int`, `echo`) must all pass on every subsequent CI run. Adding a tenth program or changing an expected exit code requires a deliberate act.

2. **`@buf-alloc` is limited to `let` RHS position.** Using `@buf-alloc` in any other expression position (app argument, match scrutinee, etc.) is a codegen error. This is an intentional restriction of ADR 0038's stack-lifetime model; it is not a bug to fix, and relaxing it requires a new ADR.

3. **`Ty::Unknown` as the buffer-arg type is a structural concession.** The typecheck crate uses `Ty::Unknown` for the buffer argument of `@read` and `@write` so both `Str` (string literals, smoke #3) and `Buf` (`@buf-alloc` handles, smoke #8) unify without type errors. Phase 3 or later may introduce a proper `BufLike` type class or subtype relation; until then `Unknown` is the bridge.

4. **The `HoleDiag` struct is advisory.** `Parser::holes` is populated during parsing but `parse_authoring` does not yet surface it to callers. Stage 5 or a future diagnostic pass should thread `holes` through to the CLI's structured-diagnostic output. This is not a bug; it is a deferral noted here so the Stage 5 freeze ADR can tick it off.

5. **`PatCtor`-with-numeric-name backward compatibility is preserved.** `compile_match` still accepts numeric `pat-ctor` names as integer literal arms. Pre-ADR-0037 canonical files in the test-vector corpus remain valid; the `PatInt` path is additive.

## Exit-gate evidence

Per [phase-2-plan.md § Stage 4](../plans/phase-2-plan.md):

> Exit gate: nine-program smoke corpus runs end-to-end on `ubuntu-latest` CI; the round-trip property in `tacit-views` covers the previously excluded `module`-bearing fixtures; a parser-error fixture demonstrates `Hole` flowing through to a structured-JSON diagnostic without a hard fail.

- **Nine-program corpus.** `cargo test --features tacit-codegen/llvm19-1,tacit-cli/llvm19-1` passes all 9 smoke tests. ✓
- **Round-trip covers `module`.** `28-module-one-binding.canonical`, `32-pat-int-match.canonical`, `33-buf-alloc-read.canonical` removed from SKIP; `authoring_round_trip_all_canonical_vectors` passes. ✓
- **Hole flows through without hard fail.** `neg_parser_recovery_hole_flows_through_typecheck` parses `lambda x. => x` as `Ok`, finds `hole` in the canonical output, and `infer_module` returns `Err` with a hole diagnostic. ✓
- **Full test suite clean.** `cargo clippy --all-targets --features tacit-codegen/llvm19-1,tacit-cli/llvm19-1 -- -D warnings` passes with zero warnings. ✓

## What is NOT frozen (Stage 5 work)

- `tacit-cli` wiring: `tacit compile` running `infer_module` before codegen; `tacit check` subcommand.
- `--types` / `--effects` flags in `tacit view --as inspection`.
- `docs/compiler-architecture.md` update for `tacit-typecheck` in the dependency graph.
- `HoleDiag` surfacing through `parse_authoring` return value to CLI diagnostics.
- `docs/error-format.schema.json` JSON Schema file (ADR 0041 deliverable).
- The Phase 2 freeze ADR (mirrors ADR 0033 for Phase 1).

## Related decisions

- [ADR 0033](0033-phase-1-frozen.md) — Phase 1 freeze; § 3 lists the four carry-overs closed here.
- [ADR 0037](0037-p2-pat-int.md) — `pat-int` spec; implemented in this stage.
- [ADR 0038](0038-p2-writable-buffer.md) — writable-buffer model; implemented in this stage.
- [ADR 0039](0039-p2-module-authoring-syntax.md) — `module` authoring syntax; implemented in this stage.
- [ADR 0040](0040-p2-hole-recovery.md) — hole-node recovery; implemented in this stage.
- [ADR 0044](0044-p2-stage-1-frozen.md) — Stage 1 spec surface that this stage builds against.
