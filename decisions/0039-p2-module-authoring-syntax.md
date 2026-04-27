# 0039 — Phase 2 top-level `module` authoring syntax

**Status:** Accepted
**Date:** 2026-04-27
**Phase:** 2, Stage 1
**Closes:** [phase-2-plan.md Q-P2-6](../plans/phase-2-plan.md); [phase-1-plan.md § Stage 2](../plans/phase-1-plan.md) exclusion for top-level `module` authoring syntax

## Context

The canonical `module` node (`(module binding₀ binding₁ …)`) has been
in the frozen canonical format since [ADR 0004](0004-rec-arity.md): N
recursively-bound definitions, no body. The inspection view already renders
`module` nodes (Phase 1 Stage 3). What has never existed is an
*authoring-view* surface for writing a `module` block.

[phase-1-plan.md § Stage 2](../plans/phase-1-plan.md) explicitly excluded
`module` authoring syntax:

> Top-level `module` form is not in scope for Stage 2. The canonical
> `module` kind exists (ADR 0004) and the inspection view renders it;
> only the authoring-view projection is held back until Phase 2.

The round-trip test in `crates/tacit-views/tests/round_trip.rs` has a
corresponding skip-list entry for `module`-bearing fixtures
(`28-module-one-binding.canonical`).

Phase 2 needs `module` authoring syntax for two reasons:
1. **Export boundary annotations.** Per the Phase 2 plan, exported
   definitions in a `module` must carry explicit type and effect signatures.
   The authoring view needs syntax for these signatures.
2. **Smoke corpus and Phase 2 example programs.** Programs beyond simple
   `rec` bodies need a `module` as their top-level form.

This ADR specifies the authoring surface. The canonical form is frozen and
unchanged; only the parse direction (authoring text → canonical AST) is new.

## Decision

**The authoring view adds a `module { … }` block as the top-level form for
a file that contains mutually recursive, named definitions. Each binding
carries an optional type+effect annotation. Unannotated exported definitions
are permitted in Phase 2 but generate a `module-missing-annotation` warning
(not an error) until the typechecker infers and records their signatures.**

### Authoring syntax

```
module {
  name₀ : type-sig₀ = expr₀ ;
  name₁ : type-sig₁ = expr₁ ;
  …
  nameₙ = exprₙ           ← annotation optional
}
```

Rules:
- The `module` keyword is the first token in the file (no preamble).
- `{` opens the binding list; `}` closes it.
- Bindings are separated by `;`. A trailing `;` before `}` is permitted.
- Each binding is `name = expr` (unannotated) or `name : type-sig = expr`
  (annotated). The annotation precedes the `=`.
- `name` is an authoring-view identifier (follows the authoring grammar's
  identifier rules; the canonicalizer resolves it to a DeBruijn index per
  [ADR 0007](0007-debruijn-rec-indexing.md) and stores the name in the
  sidecar per [ADR 0014](0014-sidecar-format.md)).
- All bindings are mutually recursive (same semantics as canonical
  `module`; per ADR 0004, the whole block is a single simultaneous frame).
  A binding may refer to any other binding in the same module by name; the
  authoring parser resolves names to DeBruijn indices.
- No `import` syntax in Phase 2. Cross-module references are deferred.
- No explicit visibility/export annotations in Phase 2. All bindings are
  exported (consistent with canonical `module`, which has no visibility
  concept in Phase 2).

### Type-signature syntax (`type-sig`)

The type-signature in `name : type-sig = expr` uses the authoring-view
type expression syntax derived from [ADR 0034](0034-p2-type-subset-ann.md),
[ADR 0035](0035-p2-effect-set-canonical.md), and [ADR 0036](0036-p2-effect-polymorphism-syntax.md):

- Base types: `Int`, `Bool`, `Str`, user-defined names (capitalized).
- Function types: `a -> b` or `a -> b / e` where `e` is an effect set
  or effect variable.
- Effect sets: `{}` (pure), `{IO}`, `{IO, Mut}`, etc.
- Effect variables: lowercase identifier after `/`.
- Record types: `{field₀: type₀, field₁: type₁}`.
- Generic types: lowercase identifiers in type position are type
  variables, implicitly universally quantified at the outermost `forall`
  bounding the signature. The authoring parser collects free type and
  effect variables and wraps the signature in the appropriate `forall`.

Example:
```
module {
  map : (a -> b / e) -> List a -> List b / e = …
  id  : a -> a = lambda x. x ;
  answer : Int = 42
}
```

Lowers to canonical (schematic):
```
(module
  (ann … (forall 2 1 (fn-ty (fn-ty (ty-var 0) (ty-var 1) (eff-var 0))
                            (fn-ty (app (sym List) (ty-var 0))
                                   (app (sym List) (ty-var 1))
                                   (eff-var 0))
                            (eff-var 0))))
  (ann (lam (var 0)) (forall 1 0 (fn-ty (ty-var 0) (ty-var 0) (eff-set))))
  (int 42))
```

An annotated binding's RHS in canonical form is `(ann expr type-sig)`. An
unannotated binding's RHS is the expression directly.

### Round-trip behavior

The authoring ↔ canonical round-trip for `module` is *losssy in the
annotation direction*: the canonical form preserves the annotation as an
`ann` node; the authoring view renders the annotation back from the `ann`
node's type child. Name recovery from the sidecar is unchanged (per ADR 0014).

The skip-list entry for `28-module-one-binding.canonical` in
`round_trip.rs` is removed when Stage 4 implements the authoring parser
for `module`. The fixture passes the round-trip property once the
authoring parser can produce and consume `module` nodes.

### Effect on the Phase 2 exit criterion

The Phase 2 exit criterion requires that "exported definitions in a `module`
carry explicit type+effect signatures" (per phase-2-plan.md Stage 2). This
ADR provides the syntactic surface for those signatures. The typechecker
(Stage 2) enforces the policy — the authoring parser does not error on
missing annotations, but the typechecker does warn (and at the module
boundary, warns as `module-missing-annotation` per ADR 0041).

## Alternatives considered

- **`module` keyword without braces; binding-per-line.** Would require
  significant disambiguation in the authoring grammar (how does the parser
  know where a binding ends and the next begins without a separator?).
  The `{…; …}` structure is unambiguous and consistent with the
  `rec { … }` authoring syntax already in Phase 1. Rejected: whitespace-
  sensitive syntax is harder to specify canonically.

- **`module Name { … }` with a required name.** Module names are a cross-
  module concern; Phase 2 has no cross-module references. Requiring a name
  where there is no semantic use of it would add surface that the
  toolchain ignores. The name can be added in Phase 3 when cross-module
  resolution is designed. Rejected for Phase 2.

- **`export name = expr` declarations without a `module` wrapper.** Files
  would be "implicitly" modules. The canonical `module` node is already
  the wrapper; making it implicit in the authoring view means the parse-to-
  canonical mapping has an invisible node. Rejected: every canonical AST
  node should have a corresponding authoring surface.

- **Separate `pub` modifier per binding.** `pub name = expr` for exported,
  `name = expr` for private. Phase 2 has no private bindings (all are
  exported). Adding `pub` now anticipates a privacy distinction that isn't
  designed yet. Rejected as premature.

- **Require explicit `forall` in type signatures.** `id : forall a. a -> a`
  instead of `id : a -> a`. More explicit about quantification boundaries,
  but more verbose for the common case and inconsistent with the
  conventional surface of ML/Haskell-style type annotations. The canonical
  `forall N M body` is already explicit; the authoring view can elide the
  `forall` keyword by collecting free variables implicitly. Rejected for
  authoring; the canonical form is already explicit.

## Consequences

- The `module` authoring parser path in `tacit-views::authoring` is
  implemented in Stage 4.
- The skip-list entry in `round_trip.rs` for `module`-bearing fixtures
  (`28-module-one-binding.canonical`) is removed in Stage 4.
- Phase 2 example programs under `examples/phase-2/` use `module { … }`
  as their top-level form and carry explicit annotations.
- The Phase 2 smoke programs that currently use `rec` bodies at the
  top level may optionally be migrated to `module` form; the existing
  seven programs are not required to change (the Phase 1 regression
  baseline is preserved per ADR 0033).
- `tacit-cli`'s `tacit view --as authoring` and `tacit view --as inspection`
  both gain `module` round-trip support in Stage 4/5.
- The authoring parser's name-resolution path is extended to handle
  mutual recursion across `module` bindings (same algorithm as `rec`,
  per ADR 0007, but at file scope).

## Related decisions

- [ADR 0004](0004-rec-arity.md) — canonical `module` kind (N bindings,
  no body); the canonical form this ADR's authoring syntax projects onto.
- [ADR 0007](0007-debruijn-rec-indexing.md) — DeBruijn convention for
  simultaneous binding; applies identically to `module` bindings.
- [ADR 0014](0014-sidecar-format.md) — sidecar stores binding names;
  `module` binding names follow the same sidecar convention.
- [ADR 0033](0033-phase-1-frozen.md) — Phase 1 baseline; the seven Phase 1
  smoke programs do not require migration to `module` form.
- [ADR 0034](0034-p2-type-subset-ann.md) — type-signature syntax used in
  module binding annotations.
- [ADR 0035](0035-p2-effect-set-canonical.md) — effect-set syntax in
  module annotations.
- [ADR 0036](0036-p2-effect-polymorphism-syntax.md) — effect variable
  syntax in module annotations.
- [phase-2-plan.md Q-P2-6](../plans/phase-2-plan.md) — closed.
