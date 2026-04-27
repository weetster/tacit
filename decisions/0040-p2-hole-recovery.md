# 0040 — Phase 2 hole-node parser recovery

**Status:** Accepted
**Date:** 2026-04-27
**Phase:** 2, Stage 1
**Closes:** [phase-2-plan.md Q-P2-7](../plans/phase-2-plan.md)
**Supersedes:** [ADR 0023](0023-hole-node-recovery-deferred.md)
**Amends:** [ADR 0013](0013-canonical-text-format-frozen.md) — additive (new diag-ids in § 7 table)

## Context

[ADR 0023](0023-hole-node-recovery-deferred.md) deferred hole-node parser
recovery from Phase 1, documenting the rationale: no Phase 1 consumer
would benefit from a partial AST. Phase 2 changes this — the typechecker
(`tacit-typecheck`) and the structured error format ([ADR 0041](0041-p2-structured-error-format.md))
are both concrete consumers that need malformed-subtree recovery to operate
usefully. A typechecker that hard-fails at the first parse error cannot
produce type or effect diagnostics for the rest of the file.

The AST enum already carries a `Hole` variant ([ADR 0016](0016-rust-ast-enum-location.md)),
and the canonical format specifies `(hole DIAG-ID (str "payload"))` with
an initial set of diag-ids (canonical-text-format.md § 7). The canonical
parser, emitter, and BLAKE3 hasher already handle `Hole` nodes — explicit
holes in canonical text parse, hash, and round-trip today. What is missing
is the authoring-view parser's ability to *produce* `Hole` nodes during
recovery rather than hard-failing with `ParseError`.

This ADR specifies:
1. The recovery algorithm in the authoring parser.
2. New diag-ids (extension of canonical-text-format.md § 7's table).
3. The downstream contract for the typechecker and codegen.

## Decision

**The authoring-view parser (`tacit-views::authoring`) is extended to
recover from parse errors by emitting typed `Hole` nodes at the site of
the error rather than aborting. The canonical parser remains strict
(hard-fail on malformed canonical text); only the authoring parser
gains recovery.**

### Recovery algorithm

The authoring parser is a top-down recursive descent parser over
Tacit-Lite's authoring grammar. On encountering a token it cannot
incorporate into the current production:

1. **Record the error.** Construct a `Hole` node:
   - `diag-id`: the most specific applicable id from the table below.
   - `payload`: a human-readable description of what was expected vs.
     what was found, including the token and its position in the source.
   - Position: stored in the sidecar (not in canonical form, which is
     content-addressed; the sidecar maps source positions to hole nodes
     per [ADR 0014](0014-sidecar-format.md)).

2. **Advance to a synchronization point.** The parser skips tokens until
   it reaches one of:
   - `;` (binding separator in `module`, `rec`, `let` sequences)
   - `}` (close of a block that was explicitly opened)
   - End-of-file

3. **Resume parsing.** After the synchronization point, parsing continues
   as if the malformed construct were replaced by the `Hole` node.

The `Hole` node is placed in the position the malformed construct would
have occupied. The surrounding parse context is preserved: if the error
occurred in the pattern position of an `arm`, the arm gets a `Hole` as
its pattern; if it occurred in the RHS of a `module` binding, the binding
gets a `Hole` as its value expression.

### Recovery scope

Recovery operates at the *expression or pattern level*, not the character
level. If a single token is malformed (e.g., an invalid escape sequence
in a string literal), the entire token is replaced with a `Hole` and
parsing resumes at the next synchronization point. The parser does not
attempt to reconstruct partially-valid tokens.

Recovery does **not** apply to:
- The canonical-text parser (remains strict; malformed canonical text is
  always a hard error).
- Surrogate codepoints and out-of-range Unicode in string literals (per
  [ADR 0012](0012-unicode-scalar-value-restriction.md); these are hard
  errors at the lexer level, not recoverable).

### New diag-ids

The following ids are appended to the diag-id table in
canonical-text-format.md § 7 (Phase 2 additions, additive only):

| Diag id               | Meaning                                                           |
|-----------------------|-------------------------------------------------------------------|
| `type-parse-error`    | Malformed type expression in a type annotation (`:` position).   |
| `effect-parse-error`  | Malformed effect annotation (after `/` in a type signature).      |
| `module-binding-error`| Malformed binding inside a `module { … }` block.                 |

The Phase 1 diag-ids (`unexpected-token`, `unclosed-paren`, `expected-expr`,
`expected-pattern`, `unbound-name`, `arity-mismatch`) remain unchanged.

### Downstream contract: typechecker

The `tacit-typecheck` crate must handle `Hole` nodes in any position
without hard-failing and without cascading spurious type errors:

1. A `Hole` in *expression* position is assigned a fresh unconstrained
   type variable. The hole's diag-id and payload are forwarded to the
   error set as a `hole-diagnostic` per [ADR 0041](0041-p2-structured-error-format.md).
   No further type errors are attributed to the hole's position.
2. A `Hole` in *type* position (inside an `ann` node's type child, or in
   a module binding annotation) is treated as an unknown type. The
   typechecker may still infer the type from usage; if inference fails
   because of the hole, a `type-parse-error` diagnostic is emitted for
   the position.
3. A `Hole` in *pattern* position (inside an `arm` pattern) is treated as
   a wildcard — it binds nothing and matches anything. The hole diagnostic
   is forwarded; no cascading pattern-exhaustiveness errors are attributed
   to the hole.

After typechecking, if any `Hole` node exists in the AST, the
type-and-effect checker marks the module as *incomplete*. The diagnostics
are collected and emitted to the error output; the module is not passed to
codegen.

### Downstream contract: codegen

Codegen (`tacit-codegen`) is **never invoked on an AST containing `Hole`
nodes.** The pipeline gate in `tacit-cli`:

```
parse → (recover) → typecheck → [if no holes] → codegen
```

If any `Hole` node is present after parsing (before or after typecheck),
codegen is skipped. The exit code for a compile with holes is
distinct from a clean typecheck failure: it is the "parse error with
recovery" exit code rather than the "type error" exit code.

### Sidecar integration

Source positions (line/column) for hole nodes are stored in the sidecar
alongside the sidecar's existing display-name and comment entries. The
sidecar format ([ADR 0014](0014-sidecar-format.md)) is extended with a
`hole_positions` table mapping hole BLAKE3 hashes to `{line, col}` pairs.
The canonical `(hole DIAG-ID (str "…"))` is content-addressed by its
bytes; the sidecar maps from hash to source position. This extension does
not change the canonical format; it is a sidecar-format amendment.

## Alternatives considered

- **Recover in the canonical parser as well.** The canonical parser is
  strict by design — it validates canonical text, which is machine-
  generated. Machine-generated canonical text should never be malformed;
  if it is, hard-failing is the correct behavior. The authoring parser
  exists to handle human and AI input, which can be partially formed.
  Rejected: canonical parser stays strict.

- **Skip-to-next-binding instead of skip-to-synchronization-point.** A
  coarser recovery that always skips to the next `;` or `}`. Would lose
  partial information within an expression. The synchronization-point
  approach is the same coarseness but is stated more precisely.
  Accepted.

- **Top-level-only recovery (one `Hole` per malformed top-level
  binding).** Rejected in ADR 0023 as "worst of both options." Full
  expression-level recovery was the intended design; this ADR delivers it.

- **Hole nodes carry a full structured parse state as payload.** The
  payload could be a canonical representation of the token stream that
  failed. Rejected: the payload is human-readable text (per the existing
  canonical-text-format.md § 7 design). Structured parse state belongs in
  the sidecar, not in canonical text.

- **Propagate type errors through holes (treat hole as the bottom type).** 
  Would allow the typechecker to keep running but would produce cascading
  errors wherever the hole's inferred type is used. The "fresh unconstrained
  type variable" approach is more informative: it signals "unknown, not
  wrong." Accepted.

## Consequences

- The authoring-view parser is extended in Stage 4 of Phase 2.
- Phase 1's hard-fail behavior for authoring-view parse errors is replaced
  by recovery + `Hole` nodes. Programs that were previously rejected with
  `ParseError` now produce a partial AST with diagnostics.
- The `tacit check` subcommand ([ADR 0041 context](0041-p2-structured-error-format.md))
  works on programs with holes; `tacit compile` does not (gate in CLI).
- The round-trip property for the authoring ↔ canonical direction remains
  *lossy through `Hole` nodes* — a `Hole` round-trips as a `Hole` in the
  canonical direction, but the authoring view cannot reconstruct the
  original malformed source from a `Hole` node. This was already the
  Phase 1 behavior for explicitly-constructed `Hole` nodes in canonical
  text.
- Canonical-text-format.md § 7 gains three new rows; no existing row
  changes.
- The sidecar format gains a `hole_positions` table.
- The inspection view renders `Hole` nodes with their diag-id and payload
  in all layers (L0/L1/L2). This rendering already works in Phase 1
  (the Phase 1 inspection view handles `Hole` from the AST enum); no
  Stage 5 work is needed for this specific case.

## Related decisions

- [ADR 0013](0013-canonical-text-format-frozen.md) — amended: new
  diag-ids in § 7 table.
- [ADR 0014](0014-sidecar-format.md) — amended: `hole_positions` table
  added to sidecar format.
- [ADR 0016](0016-rust-ast-enum-location.md) — `Hole` AST variant
  already present; unchanged.
- [ADR 0023](0023-hole-node-recovery-deferred.md) — superseded by this ADR.
- [ADR 0041](0041-p2-structured-error-format.md) — structured error
  format; `hole-diagnostic` kind forwards `Hole` payloads.
- [phase-2-plan.md Q-P2-7](../plans/phase-2-plan.md) — closed.
