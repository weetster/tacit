# 0004 — `rec` arity: 1+N with a separate `module` kind

**Status:** Accepted
**Date:** 2026-04-21
**Phase:** 0, Stage 1 (closing the last Stage 2 prerequisite)

## Context

The draft kind tables in [authoring-sexpr-int-ids.md](../plans/candidates/authoring-sexpr-int-ids.md) and [authoring-glyph-prefix.md](../plans/candidates/authoring-glyph-prefix.md) list `rec` as arity **N** — a bundle of N recursive binding RHSes, no body. The 100-node Sample 2 in [reference-ast.md](../plans/candidates/reference-ast.md) used `rec` as **1+N** — N bindings followed by a body that uses them in scope — because otherwise the mutually-recursive `length`, `isEmpty`, and `head` bindings had nowhere to be called from.

All five authoring-view encodings render either shape at the same token cost, so this is not a density question. It is a spec question about what the `rec` AST node actually is. Stage 2 (canonical text format) cannot freeze the grammar while `rec` means two different things in two different docs.

## Decision

**Inner `rec` has arity 1+N: N binding RHSes followed by a body that sees the bindings in scope.** This matches Sample 2's usage and generalizes `let` (arity 2 = 1 rhs + 1 body) to `rec` (arity 1+N = N rhses + 1 body). A `rec` subtree is self-contained: its bindings and all direct uses of them are one contiguous tree, which is what the parent plan's "rec groupings hash as a single atom" commitment requires.

**Top-level modules are a separate kind.** A module file is a set of recursively-bound definitions with no body — the `N`-arity shape the draft kind table was describing. Rather than overload `rec` to mean two different shapes depending on position, introduce a distinct `module` kind:

- `rec` (inner): arity 1+N. N bindings + 1 body. Body is the last child.
- `module` (top-level): arity N. N recursively-bound definitions, no body. Only appears at a file root.

Both hash as single atoms per the parent plan's commitment. `module` details (name resolution, export semantics, how multiple files compose) are deferred — Phase 0 does not exercise them, and Sample 2 is entirely inner-`rec`. The point here is to reserve the kind so `rec`'s shape is unambiguous.

## Alternatives considered

- **Keep `rec` at arity N, add a sequencing kind for inner use.** Forces every inner recursive group to be wrapped in a `let _ = rec { ... } in body` or equivalent. Adds a node to every inner use for no structural benefit, and loses the `let`/`rec` symmetry. Rejected.
- **Keep `rec` at arity N and treat the body as an implicit continuation in the surrounding context.** Breaks self-contained hashing — the "rec group" atom would not include what uses it, so two programs that differ only in the body would share the rec's hash. Rejected; violates the parent plan's commitment.
- **Make `rec` polyarity (N *or* 1+N depending on position).** Saves one kind at the cost of a context-sensitive grammar. Canonicalizers would need to know whether they're inside a module context to know how many children to expect. Rejected; context-sensitivity in the AST shape is exactly what canonical text is supposed to avoid.
- **Use `module` as the only form and forbid inner `rec`.** Collapses the problem by removing mutual recursion below the top level. Unacceptable — mutual recursion in inner scopes is a real use case (Sample 2 demonstrates it), and forbidding it would push users to hoist recursive groups to module level, inflating scopes.

## Consequences

- **Kind tables in both draft docs are updated** to list `rec` as arity 1+N and add a `module` row at arity N. Keeping the two docs in sync is a mechanical update; no grammar re-work.
- **Sample 2's rendering is now canonical**; no re-scoring is needed. All five encodings in reference-ast.md already render `rec` as 1+N, so the Q1 result ([ADR 0003](0003-authoring-view-bpe-compact.md)) is unaffected.
- **Stage 2 canonical spec is unblocked.** The `rec`/`module` distinction, the 1+N body position, and the "hash as single atom" rule are now written down precisely enough to specify byte-exact canonical rendering.
- **`module` is reserved but not specified.** Phase 0 does not define its semantics beyond "N recursively-bound definitions, no body, hashes as a single atom." Full module semantics (exports, imports, file composition) are Phase 1+ work and will need their own ADR when they become concrete.
- **DeBruijn depth under `rec`**: the N bindings all see each other, so each RHS has N names in scope at its own depth-0…N-1 slots; the body sees the same N names. This is already consistent with Sample 2's DeBruijn sketch in reference-ast.md; the canonical spec will need to write the exact depth-assignment rule down when Stage 2 freezes the grammar.
- **Parallel to `match`** (arity 1+N = scrutinee + N arms) is a coincidence of shape, not of semantics. No structural sharing between the two kinds is intended.

## Related decisions

- [ADR 0003](0003-authoring-view-bpe-compact.md) — authoring view format; its Sample 2 rendering assumes 1+N, which this ADR ratifies.
- Supersedes the arity cell in the Kind tables in [authoring-sexpr-int-ids.md](../plans/candidates/authoring-sexpr-int-ids.md) and [authoring-glyph-prefix.md](../plans/candidates/authoring-glyph-prefix.md); those docs are updated to match.
