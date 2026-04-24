# 0026 — Phase 1 closure representation: closed lambdas, top-level monomorphic lowering

**Status:** Accepted
**Date:** 2026-04-24
**Phase:** 1, Stage 4
**Closes:** [phase-1-plan.md Q-P1-3](../plans/phase-1-plan.md)

## Context

The canonical AST has `Lam` and `App` nodes; canonical text can express
first-class lambdas with free variables (captures over enclosing
binders, referenced through DeBruijn indices that step past the
`Lam` binder itself).

[phase-1-plan.md § Stage 4](../plans/phase-1-plan.md) poses Q-P1-3 as
the choice between:

- **(A)** Ban free variables in lambdas for Phase 1 — lower `Lam` as
  a top-level monomorphic function, `App` as a direct call.
- **(B)** Emit a closed-over environment struct now, pinning the
  closure ABI in Phase 1.

A real closure ABI has knock-on decisions: environment layout
(heap vs. stack), capture discipline (by-value vs. by-reference),
lifetime of captured bindings, whether closures are first-class
values that flow through records and function parameters, and
eventual integration with the ownership and effect systems Phase 2
introduces. None of those decisions have Phase 1 evidence to ground
them — the smoke corpus is hello-world-shaped, and exercising any
specific closure ABI choice against that corpus would not discriminate
between alternatives.

Phase 1's goal is the pipeline, not the language. Committing to a
closure ABI now means committing in the absence of the exact
information (types, effects, ownership) that should inform the
choice.

## Decision

**Phase 1 lambdas must be closed: no free variables referring outside
the lambda's own parameter binder. The codegen lowers each `Lam` as a
top-level monomorphic LLVM function and each `App` as a direct call.
First-class function values are not supported in Phase 1.**

Concretely:

1. **Free-variable check.** A codegen pre-pass walks each `Lam` body
   and computes the set of free DeBruijn indices (those pointing to
   binders above the `Lam` itself). If non-empty, codegen emits a
   structured `CodegenError::FreeVarInLambda { index, location }`
   and aborts the compilation unit. The check uses the existing
   DeBruijn machinery from the canonicalizer.
2. **Closed-`Lam` lowering.** Each closed `Lam` becomes a uniquely-
   named LLVM function at module scope. The synthetic name is derived
   deterministically from the `Lam`'s content hash plus a per-module
   disambiguator (exact format pinned in Stage 4 work; this ADR
   commits only to "deterministic and collision-free").
3. **`App` lowering.**
   - `App(Lam(body), arg)` → direct call to the lowered function for
     that `Lam`.
   - `App(Var(i), arg)` where resolving `Var(i)` reaches a
     `Let`/`Rec`-bound `Lam` → direct call to that `Lam`'s lowered
     function.
   - `App(Var(i), arg)` where `Var(i)` resolves to a non-lambda
     value → `CodegenError::AppNonFunction { location }`.
4. **First-class function values banned.** A `Var` whose binding is
   a `Lam` may only appear in `App` position (as the function being
   applied). It cannot be passed as an argument, stored in a
   `Record`, returned from a non-`Lam` computation, or otherwise
   treated as a runtime value. Violations produce
   `CodegenError::FirstClassFunction { location }`.
5. **AST unchanged.** Canonical text containing free-variable
   lambdas or first-class function values still parses, hashes, and
   round-trips through the views. The restriction is enforced at
   codegen time only, not at parse or canonicalisation time.

## Alternatives considered

- **Emit a closure environment struct in Phase 1.** Rejected. Commits
  to a closure ABI without Phase 2's type system to validate it. The
  ABI choices (heap vs. stack env, value vs. reference capture,
  ownership integration) interact with work Phase 2 owns; choosing
  now means re-choosing later with higher migration cost, because
  any existing smoke programs using closures would be written against
  the Phase 1 ABI.
- **Function pointers without environments (first-class but
  non-capturing).** Rejected. A partial closure ABI that is awkward
  to extend — either the codegen has a full closure abstraction or it
  does not. Phase 2 can adopt closures cleanly without inheriting a
  function-pointer legacy format.
- **Forbid `Lam` entirely; require all functions to be top-level
  `Rec`-bound.** Rejected. Closed `Lam` is trivial to lower and
  convenient for writing smoke programs (e.g., `let f = lam x. ... in ...`).
  Forbidding it leaks lowering details into source structure with no
  codegen simplification.
- **Accept free-variable lambdas but error at link time instead of
  codegen time.** Rejected. The check is cheap at codegen time and
  the error is more useful when attributed to a specific AST
  location than to an unresolved symbol.

## Consequences

- Stage 4 codegen is dramatically simpler. No environment struct,
  no indirect calls, no closure ABI to design.
- Phase 1 smoke programs cannot demonstrate higher-order functions
  over user-defined operations (no `map`/`filter`/`fold` with a
  Tacit-written callback). Accepted scope — Phase 1's exit criterion
  is "pipeline works," not "language is complete."
- Phase 2 (or Phase 6, depending on demand) inherits the closure-ABI
  design as a known follow-up. The AST is already closure-ready; only
  the codegen needs to grow.
- Programs written for Phase 1's smoke corpus compile unchanged when
  closures are added in a later phase. The restriction is
  additive-only — lifting it does not invalidate existing programs.
- `CodegenError::FreeVarInLambda`, `CodegenError::FirstClassFunction`,
  and `CodegenError::AppNonFunction` become part of the Stage 4
  diagnostics surface. Small but non-trivial work item; their
  messages should include the DeBruijn-index-to-display-name
  projection so the error is legible in authoring view.
- The content-hash-derived function-name scheme keeps incremental
  compilation and content-addressed caching open as future options
  without committing to either now.

## Related decisions

- [ADR 0007](0007-debruijn-rec-indexing.md) — DeBruijn indexing
  convention; the free-variable check walks indices under this
  convention.
- [ADR 0024](0024-llvm-bindings-inkwell.md) — `inkwell` is the API
  used to emit the top-level function and direct call.
- [ADR 0027](0027-phase-1-rec-lowering.md) — uses the same C calling
  convention for all Phase 1 functions, including `Lam`-lowered ones.
- [phase-1-plan.md § Stage 4, § Open Questions Q-P1-3](../plans/phase-1-plan.md)
  — closed by this ADR.
- Future closure-ABI ADR (number TBD; expected Phase 2+) — will lift
  this restriction, design the environment lowering, and specify
  first-class function values.
