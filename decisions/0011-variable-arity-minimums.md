# 0011 — Minimum arity for variable-arity kinds

**Status:** Accepted
**Date:** 2026-04-21
**Phase:** 0, Stage 2

## Context

The § 2 node-kinds table in `canonical-text-format.md` uses arity expressions `2N`, `1+N`, and `N` for variable-arity kinds without stating the minimum value of N. Stage 2 test-vector drafting (`plans/test-vectors-draft.md`, vector 10) surfaced six ambiguous N = 0 cases:

1. `(record)` — empty record, 2N with N = 0.
2. `(ctor Nil)` — nullary constructor, 1+N with N = 0.
3. `(pat-ctor Zero)` — nullary constructor pattern, 1+N with N = 0.
4. `(rec body)` — body-only `rec`, 1+N with N = 0.
5. `(match scrutinee)` — match with no arms, 1+N with N = 0.
6. `(module)` — empty module, N = 0.

Byte-exact canonicalization requires a definite rule for each: either the canonicalizer produces these bytes (permitted) or the form is structurally forbidden and the parser is responsible for never generating the AST that would produce it.

## Decision

**Permit N = 0 where the construct has a meaningful empty form; forbid otherwise.** Per kind:

| Kind       | Arity | Min N | N = 0 canonical       | Permitted? |
|------------|-------|------:|-----------------------|------------|
| `record`   | 2N    |     0 | `(record)`            | **yes** — the unit record is a meaningful value with exactly one canonical form. |
| `ctor`     | 1+N   |     0 | `(ctor Nil)` etc.     | **yes** — nullary constructors (`Nil`, `None`, `Unit`) are essential. |
| `pat-ctor` | 1+N   |     0 | `(pat-ctor Zero)` etc.| **yes** — nullary constructor patterns mirror nullary `ctor`. |
| `rec`      | 1+N   |     1 | `(rec body)`          | **no** — semantically equal to `body`; two canonical forms for one meaning. |
| `match`    | 1+N   |     1 | `(match scrutinee)`   | **no** — non-exhaustive by construction; never matches. |
| `module`   | N     |     1 | `(module)`            | **no** — no content; nothing to name-resolve against. |

The § 2 kind table updates to reflect these minimums inline (e.g. `1+N, N≥1` for `rec`, `match`, `module`; `2N, N≥0` for `record`; `1+N, N≥0` for `ctor`, `pat-ctor`).

## Alternatives considered

- **Permit all N = 0 forms uniformly.** Simpler rule. Rejected for `rec`: `(rec body)` and `body` would hash differently despite denoting the same program, violating content-addressing. The other forbidden cases (`module`, `match` with no arms) fail the same principle in a softer form — canonical forms reachable only by deliberate malformation, not by any meaningful source program. The canonicalizer's job is to serialize ASTs the parser produces; forms outside that set need not be accepted.
- **Forbid all N = 0 forms uniformly.** Would mean no nullary constructors — unacceptable for Tacit-Lite, which needs `Nil`, `None`, and friends. Rejected.
- **Defer to Phase 1.** Tempting since the parser isn't written. Rejected: Stage 2's purpose is to make canonicalization deterministic enough that a parser *can* be written against it. Leaving N-minimums open pushes the decision onto Phase 1's implementation, reversing the spec-first commitment in CLAUDE.md.
- **Represent nullary constructors as bare `sym`.** Collapses `(ctor Nil)` into just `Nil`-as-symbol. Rejected: loses the structural distinction between "the symbol Nil" and "the constructor Nil applied to zero arguments" — matters for pattern matching, exhaustiveness analysis, and any future type-directed tooling.

## Consequences

- **§ 2 kind table becomes the authoritative arity reference** with inline N-minimums, not a separate doc or footer.
- **Vector 10 splits.** 10a (`(record)`) and 10b (`(ctor Nil)`) are canonical and valid. 10c (`(module)`), 10d (`(rec body)`), 10e (`(match scrutinee)`) are anti-tests — the canonicalizer must not emit them, and a compliant parser must not produce an AST that would serialize to one of them. The test-vector file retains them as negative examples.
- **Parser behavior for source-level "empty rec/module/match" is a Phase 1 concern.** This ADR tells the parser what *not* to produce; exactly how source-level `rec { }` or `match x with { }` is handled (rejection, hole, silent simplification) is Phase 1's call.
- **No change to the surface form, lexical rules, or hashing rule.** This is a constraint on which ASTs exist, not on how existing ASTs serialize. ADRs 0005, 0006, 0009 are untouched.
- **Additive-only kind-table evolution (per ADR 0005) still holds.** Any new kind added in Phase 1+ must specify its N-minimum in its introducing ADR.

## Related decisions

- [ADR 0004](0004-rec-arity.md) — established `rec` arity as 1+N and introduced `module` at arity N. This ADR constrains the minimum N for both.
- [ADR 0005](0005-canonical-surface-form.md) — defines the kind table that this ADR constrains.
- [ADR 0006](0006-canonical-lexical-rules.md) — parser-level validity; orthogonal to this ADR's structural constraint.
- [ADR 0010](0010-canonical-emission-rules.md) — parallel pinning for atom emission, landing at the same time.
