# 0030 — Phase 1 arithmetic and comparison primitives: codegen intrinsics under `@name`

**Status:** Accepted
**Date:** 2026-04-25
**Phase:** 1, Stage 4
**Closes:** Spec gap surfaced by [phase-1-plan.md § Stage 4 + Appendix B](../plans/phase-1-plan.md) smoke corpus.

## Context

[phase-1-plan.md § Stage 4](../plans/phase-1-plan.md) lists "integer
arithmetic on `i64`" as required codegen coverage and Appendix B's smoke
corpus #2, #4, #5, #6 exercise it (`return-computed.tac`,
`if-branch.tac`, `factorial.tac`, `even-odd.tac`). [ADR 0028](0028-phase-1-libc-call-surface.md)
fixed the source-level surface for primitive calls (`@name` →
`(sym name)` at `App` head) but its allowlist is libc-only:
`{"write", "read", "exit"}`. Tacit-Lite has no built-in operators in
either the canonical text format or the authoring view grammar, so
arithmetic and comparison have no path from source to LLVM IR today.

The smoke corpus also needs `if` to consume a value of unspecified
truth shape — Tacit-Lite has no boolean type. `if n then ... else ...`
in the factorial sketch in [phase-1-plan.md Appendix B](../plans/phase-1-plan.md)
is well-defined only if `if`'s condition has a pinned interpretation.

This ADR fills the gap discovered while drafting Stage 4 codegen, per
[CLAUDE.md § Ground rules](../CLAUDE.md): spec ambiguities found in
Phase 1 are bugs against Phase 0, resolved via ADRs against the
relevant frozen artifact. The canonical text format itself is
unaffected; this ADR extends the curated `@name` surface that ADR 0028
established and pins `if`'s truthy-zero semantics.

## Decision

**Phase 1's `@name` primitive surface is extended with codegen-intrinsic
arithmetic and comparison operators that lower as direct LLVM
instructions (no libc dependency). `if`'s condition is interpreted as
`i64`: zero is false, non-zero is true.**

Concretely:

### Arithmetic primitives (binary, `i64 → i64 → i64`)

| `@name`  | LLVM lowering | Notes |
|---------|---------------|-------|
| `@add`  | `add nsw`     | Signed wrap-on-overflow undefined; matches LLVM's standard signed-arith semantics. |
| `@sub`  | `sub nsw`     | "                                                                  |
| `@mul`  | `mul nsw`     | "                                                                  |
| `@div`  | `sdiv`        | Signed integer division. Division by zero is UB at LLVM level; Phase 1 does not check. |
| `@mod`  | `srem`        | Signed remainder; sign follows the dividend (LLVM `srem` semantics). |

Overflow checking, division-by-zero trapping, and unsigned variants are
deferred to Phase 2+; this ADR commits Phase 1 to LLVM's default
`-O0` semantics without runtime guards.

### Comparison primitives (binary, `i64 → i64 → i64`)

Each emits an `icmp` followed by `zext i1 → i64`, so the result is an
`i64` value — `0` for false, `1` for true. This keeps the Phase 1 type
discipline at "everything is `i64`" without introducing a separate
boolean kind.

| `@name` | LLVM lowering              |
|--------|----------------------------|
| `@eq`  | `icmp eq` + `zext`         |
| `@ne`  | `icmp ne` + `zext`         |
| `@lt`  | `icmp slt` + `zext`        |
| `@le`  | `icmp sle` + `zext`        |
| `@gt`  | `icmp sgt` + `zext`        |
| `@ge`  | `icmp sge` + `zext`        |

### `if` truthy semantics

`(if cond then else)` lowers as: evaluate `cond` to `i64`, branch on
`icmp ne cond, 0`. Non-zero takes the `then` branch; zero takes the
`else` branch. This makes `(if n then ... else ...)` for an `i64` `n`
behave as the smoke corpus's factorial / even-odd programs assume,
without requiring a boolean type.

### Allowlist structure

Phase 1's `@name` allowlist is now the union of three disjoint sets:

- **`LIBC`** (ADR 0025): `write`, `read`, `exit` — direct external call.
- **`ARITH`** (this ADR): `add`, `sub`, `mul`, `div`, `mod` — direct
  LLVM instruction.
- **`CMP`** (this ADR): `eq`, `ne`, `lt`, `le`, `gt`, `ge` — `icmp` +
  `zext`.

A `Sym(name)` at `App` head whose name is outside the union still
fails with `CodegenError::UnknownPrimitive { name, span }` per ADR
0028. Codegen recognises which set the name belongs to and emits
accordingly.

### Effect signatures

Arithmetic and comparison primitives are **pure compute** — they carry
the empty effect set `{}` and do not appear in
`stdlib/libc-effects.toml`. The libc-effects table stays at three
OS-boundary entries per ADR 0025. Phase 2's effect checker reads
arithmetic-primitive signatures from a future stdlib effect table or
from inline codegen knowledge; this ADR does not pre-commit either.

### Arity

All ten `@name` operators here are exactly binary. Underapplication
(`@add 1`) and overapplication (`@add 1 2 3`) fail at codegen with
`CodegenError::PrimitiveArity { name, expected: 2, got: n }`.
Phase 2+ may relax this if currying or variadic primitives become
useful; Phase 1 keeps the surface mechanical.

## Alternatives considered

- **Use `(ctor add e₁ e₂)` as the canonical form for arithmetic.**
  The canonical-text-format § 10 example uses `(ctor sub (var 0)
  (int 1))` in passing, which suggests this path. Rejected:
  overloading `ctor` for built-in arithmetic conflicts with its
  semantic role (data constructor), is harder for Phase 2's effect
  checker to special-case (because `ctor` is also a user-extensible
  surface), and breaks ADR 0028's "primitives live under `@name`"
  framing.
- **Add new canonical node kinds (e.g., `(add e₁ e₂)`).** Rejected.
  The canonical text format is frozen ([ADR 0013](0013-canonical-text-format-frozen.md));
  new node kinds are spec bugs requiring a re-freeze. The `@name` →
  `(sym name)` path already exists and absorbs this naturally.
- **Implement arithmetic in Tacit-Lite itself (e.g., via a `rec`
  group of unary increment/decrement using ctor-encoded naturals).**
  Rejected as scope creep — Peano-style arithmetic at `-O0` is too
  slow for any real smoke program, and the encoding work is
  substantial. Phase 1 wants direct LLVM ops.
- **Treat arithmetic as effect-bearing for uniform codegen.** Rejected.
  Arithmetic is pure compute; conflating it with `{IO}` would
  pollute Phase 2's effect lattice and contradict
  [ADR 0025 § Pure-compute handling](0025-phase-1-libc-surface.md).
- **Emit arithmetic as LLVM intrinsics (`llvm.sadd.with.overflow.i64`
  etc.) for trap-on-overflow.** Rejected as Phase 2+ work.
  Trap-on-overflow ties into the type system's integer ranges,
  which Phase 1 lacks. Plain `add nsw` is the smallest correct
  Phase 1 choice; the overflow-trap variant is a deliberate future
  upgrade behind its own ADR.
- **Require `if` to take an explicit boolean ctor (`True` /
  `False`).** Rejected. Requires a boolean type and either user-
  defined ctors flowing through codegen (Phase 2 work) or built-in
  `True`/`False` ctors (more spec surface than the i64-non-zero
  rule). The integer-truthiness rule is the smallest viable choice
  for Phase 1; Phase 2's type system can refine it without
  invalidating Phase 1 programs (an `i64` non-zero check still works
  on a future `bool` lowered as `i64 0`/`i64 1`).

## Consequences

- Stage 4 smoke corpus #2, #4, #5, #6 become writeable in the
  authoring view: `@add`, `@sub`, `@mul`, `@lt`, etc., are the
  surface operators. Programs that need them are slightly more
  verbose than infix notation but unambiguous and stable.
- ADR 0028's "primitive namespace is curated" framing extends
  cleanly: arithmetic/comparison are stdlib-curated like libc, just
  with a different lowering target (LLVM op vs. external call).
- Phase 1 has no runtime errors for arithmetic — division by zero
  invokes LLVM's UB. Smoke programs avoid `@div 0` and `@mod 0`;
  the corpus harness does not stress these. Phase 2+ adds checked
  variants under their own ADR if needed.
- `if` semantics are pinned: integer-truthiness (zero is false). A
  program like `if n then ... else ...` is well-defined for any
  `i64 n`. The factorial smoke program in
  [phase-1-plan.md Appendix B](../plans/phase-1-plan.md) compiles
  unchanged from its sketched form.
- The Phase 1 `CodegenError` surface gains `PrimitiveArity` alongside
  `UnknownPrimitive` (ADR 0028), `FreeVarInLambda`,
  `FirstClassFunction`, `AppNonFunction` (ADR 0026), and
  `RecGroupFailed` (ADR 0027).
- `@name` results that the user wants to combine require the user
  to write nested `App` spines. A two-arg primitive lowers as
  `(app (app (sym add) e₁) e₂)`; the codegen pattern-match collects
  both right-spine arguments and emits a single LLVM instruction.
  Identical to ADR 0028's libc shape, with a different emit step.
- The arithmetic and comparison surface is fixed for Phase 1.
  Adding `@shl`, `@xor`, etc., is a one-line allowlist change
  behind a follow-up ADR; gating it preserves the same auditable
  primitive surface ADR 0028 established.

## Related decisions

- [ADR 0013](0013-canonical-text-format-frozen.md) — canonical
  text format remains untouched; `@name` → `(sym name)` path is
  the existing absorption point.
- [ADR 0025](0025-phase-1-libc-surface.md) — libc surface is
  unchanged; arithmetic/comparison live in a separate intrinsic
  set that does not appear in `libc-effects.toml`.
- [ADR 0026](0026-phase-1-closed-lambdas.md) — closed-lambda path
  remains the fallback when the `Sym` head isn't in any allowlist.
- [ADR 0027](0027-phase-1-rec-lowering.md) — C calling convention
  is irrelevant for arithmetic primitives (no call) but still
  applies to the surrounding function bodies that emit them.
- [ADR 0028](0028-phase-1-libc-call-surface.md) — extends the
  `@name` framing this ADR builds on; ADR 0028's allowlist clause
  is now read as "the union of LIBC, ARITH, CMP."
- Future Phase 2 ADR — may add overflow checking, division-by-zero
  guards, unsigned variants, or migrate arithmetic to a stdlib
  module path once module composition lands.
