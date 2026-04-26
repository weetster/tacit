# Phase 1 smoke corpus

Hand-authored programs in the authoring view that exercise the
`tacit-codegen` Stage 4 emitter. Wired into the Stage 4 exit-gate CI
job: each program canonicalizes, parses, lowers, links, and runs
under `tests/smoke.rs` with the expected stdout / exit code.

| File                | Features                                                    | Expected exit | Stdout       |
|---------------------|-------------------------------------------------------------|--------------:|--------------|
| `return-zero.tac`   | Minimal `main`; constant return.                            | 0             | (empty)      |
| `return-computed.tac` | `let`, `@mul`, `@sub` on `i64`.                           | 33            | (empty)      |
| `hello.tac`         | `@write` primitive, string literal, `app` spine.            | 0             | `hello, world\n` |
| `if-branch.tac`     | `let`, `@gt`, `if`/`then`/`else` (truthy-zero per ADR 0030).| 1             | (empty)      |
| `factorial.tac`     | Self-recursive `rec`, `@mul`, `@sub`, integer-truthy `if`.  | 120           | (empty)      |
| `even-odd.tac`      | Mutually-recursive `rec` (N=2), `@sub`, integer-truthy `if`.| 1             | (empty)      |
| `exit-nonzero.tac`  | Explicit `@exit 7` from non-`main` position.                | 7             | (empty)      |

## Deferred from Appendix B

Two programs from the original 9-program plan are deferred pending
spec work:

- **`match-int.tac`** — requires a `pat-int` canonical pattern kind.
  Canonical-text-format § 2 has no integer pattern; `pat-ctor` names
  must match `[A-Za-z_][A-Za-z0-9_-]*` so `(pat-ctor 42)` is invalid.
  Future ADR adds `pat-int` to the canonical surface; the codegen's
  `compile_match` already accepts numeric ctor names so the lowering
  is ready when the spec catches up.
- **`echo.tac`** — requires a writable-buffer binding model. Phase 1
  has no stack-allocation primitive and `let buf = "..." in body`
  cannot bind a mutable buffer pointer that survives across `@read` /
  `@write` calls. Future ADR adds either an explicit `@buffer N`
  primitive or a string-literal-as-mutable-buffer rule.

Both deferrals are recorded in the Stage 4 freeze ADR.

## Rules of the smoke corpus

- Hand-authored only — no draws from `corpus/` (sealed-hash check
  enforces this). Phase 3's evaluation set stays untouched per ADR 0020.
- One feature per program where possible.
- Deterministic output: no time, no randomness, no env reads.
- No hidden stdlib dependencies — every symbol is either a
  Tacit-Lite AST node or one of the ten Phase 1 `@name` primitives
  (LIBC ∪ ARITH ∪ CMP per ADR 0028 + ADR 0030).
