# 0027 — Phase 1 mutual recursion lowering: forward-declare under C calling convention

**Status:** Accepted
**Date:** 2026-04-24
**Phase:** 1, Stage 4
**Closes:** [phase-1-plan.md Q-P1-4](../plans/phase-1-plan.md)

## Context

The canonical AST's `Rec` node binds N mutually recursive definitions
(per [ADR 0004](0004-rec-arity.md)) and hashes as a single atom (per
[ADR 0007](0007-debruijn-rec-indexing.md)). LLVM lowering of mutual
recursion is a well-known forward-declare-then-define pattern, but the
calling convention and any tail-call optimisation (TCO) requirement
must be pinned before Stage 4 codegen work begins.

[phase-1-plan.md Q-P1-4](../plans/phase-1-plan.md) asks specifically
for the calling convention and any TCO commitment. Secondary but
related: symbol naming for Rec-group members, and the handling of
cross-module mutual recursion (which Phase 1 does not yet have a
module system for, but which Phase 2+ will need an answer for).

## Decision

**Phase 1 lowers `Rec` groups using the default C calling convention
(LLVM `ccc`), forward-declaring all N member signatures before any
body is emitted, with no tail-call optimisation guarantee.**

Concretely:

1. **Group lowering.** For each `Rec` group of N mutually recursive
   definitions:
   a. Emit N LLVM function *declarations* (signatures, no bodies) to
      the module. This makes every group member callable from every
      other member's body.
   b. Emit each function's *body*. Calls between group members lower
      as direct LLVM calls to the previously-declared symbols.
   c. Emission is atomic at the group level — a codegen error in any
      member fails the whole group with
      `CodegenError::RecGroupFailed { failing_index, cause }`, rather
      than leaving a partially-populated module.
2. **Calling convention.** LLVM's default C calling convention (`ccc`)
   for every Phase 1 function — `Rec`-group members, `Lam`-lowered
   top-level functions (per [ADR 0026](0026-phase-1-closed-lambdas.md)),
   and libc declarations (per [ADR 0025](0025-phase-1-libc-surface.md)).
   One convention everywhere keeps the codegen surface small and
   eliminates any per-call-site ABI decision.
3. **Tail calls.** No guarantee. The codegen does not annotate calls
   with `tail` or `musttail`. LLVM may still apply TCO opportunistically
   under its own heuristics, but Tacit programs in Phase 1 cannot
   rely on it — deep recursion may stack-overflow.
4. **Symbol naming.** Each member gets a deterministic synthetic name
   derived from the `Rec` group's content hash plus the member's
   positional index within the group. The exact format is pinned in
   Stage 4 work; this ADR commits to the property "deterministic,
   collision-free across compilation units, and recoverable from the
   AST without executing codegen."
5. **Cross-module Rec groups.** Not applicable in Phase 1 (no module
   system). The forward-declare pattern extends naturally across
   module boundaries when module composition lands; the
   calling-convention question may need to be revisited at that
   boundary as part of the future host-interface ADR foreshadowed in
   [ADR 0022 § 2](0022-pure-kernel-host-model.md).

## Alternatives considered

- **Guarantee tail-call optimisation via `musttail`.** Rejected as
  Phase 6 work. `musttail` has restrictive preconditions (matching
  prototypes, immediate return, specific calling conventions) that
  interact with future closure and ownership ABI decisions. Committing
  now would constrain those choices prematurely. Programs that need
  bounded-stack iteration can use explicit loops once Phase 2 exposes
  them, or wait for a dedicated TCO ADR.
- **Annotate calls with `tail` (the permissive LLVM hint).** Rejected.
  `tail` without `musttail` is a hint the optimiser may ignore; making
  Phase 1 behavior depend on optimiser mood is a worse correctness
  story than "no guarantee" plainly stated.
- **Custom calling convention (`fastcc`, `tailcc`).** Rejected.
  `fastcc` offers no measurable wins at `-O0` (Phase 1's optimisation
  level per [phase-1-plan.md § Stage 4](../plans/phase-1-plan.md));
  `tailcc` requires the TCO commitment this ADR declines; either
  choice would introduce an ABI boundary between Tacit functions and
  libc calls that C convention avoids. One convention everywhere is
  the simplest correct Phase 1 choice.
- **Inline single-use non-recursive Rec members into their callers.**
  Rejected as optimisation work that belongs in Phase 6, not in Phase
  1 codegen. The forward-declare pattern handles all Rec shapes
  uniformly.
- **Defer calling-convention choice to Phase 2.** Rejected. Codegen
  needs *some* convention to emit calls at all, and Phase 1's exit
  criterion ("hello world plus smoke corpus runs") does not leave
  room for a deferred ABI. "C convention by default" is a defensible
  Phase 1 choice that Phase 2 can revisit symbol-by-symbol if a
  non-C convention earns its way in.

## Consequences

- Stage 4 codegen for `Rec` is mechanical. Walk the group once to
  declare, again to define — both walks use the same symbol-name
  derivation, so cross-references resolve trivially.
- Self-recursion and mutual recursion share the same code path; no
  special-cased single-member lowering.
- Phase 1 programs that recurse deeply will stack-overflow. Smoke
  corpus programs are shallow, so this is not an immediate problem;
  it is a known limitation called out here so "Tacit doesn't do TCO"
  is not a surprise in Phase 2 design discussions.
- All Phase 1 calls — Tacit-to-Tacit, Tacit-to-libc, and
  `Lam`-to-`Lam` — use the same convention. Future convention
  diversity (closure ABI, host-interface ABI, eventual scratch-stdlib
  syscall shim) becomes a deliberate ADR, not an accidental fork.
- Cross-module Rec groups are deferred along with the module system
  itself. The forward-declare pattern's cross-module extension is
  sketched here so the future host-interface ADR inherits a known
  starting point, not a blank slate.
- Deterministic symbol naming preserves incremental compilation and
  content-addressed caching as future options without committing to
  either.
- `CodegenError::RecGroupFailed` joins the Stage 4 diagnostics
  surface; atomic group emission means partial-module debugging
  states are not a concern.

## Related decisions

- [ADR 0004](0004-rec-arity.md) — `Rec` arity (1+N) and the
  `module`-vs-`rec` distinction; the structural premise of this
  lowering.
- [ADR 0007](0007-debruijn-rec-indexing.md) — DeBruijn indexing for
  `Rec`; determines how intra-group references are resolved during
  codegen (index into the group's binder).
- [ADR 0024](0024-llvm-bindings-inkwell.md) — `inkwell` API used for
  the forward-declare-then-define pattern.
- [ADR 0025](0025-phase-1-libc-surface.md) — libc declarations use
  the same C calling convention this ADR pins.
- [ADR 0026](0026-phase-1-closed-lambdas.md) — `Lam`-lowered
  top-level functions use the same C calling convention this ADR
  pins.
- [phase-1-plan.md § Stage 4, § Open Questions Q-P1-4](../plans/phase-1-plan.md)
  — closed by this ADR.
- Future host-interface ADR (foreshadowed in
  [ADR 0022 § 2](0022-pure-kernel-host-model.md)) — will revisit
  calling-convention choice at the host boundary; cross-module Rec
  groups are sketched here as a known input to that work.
- Future TCO ADR (Phase 6 or when programs demonstrate the need) —
  will lift the "no guarantee" clause and specify which shapes get
  `musttail`.
