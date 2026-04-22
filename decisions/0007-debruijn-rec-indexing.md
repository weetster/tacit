# 0007 — DeBruijn indexing convention for `rec` and `module`

**Status:** Accepted
**Date:** 2026-04-21
**Phase:** 0, Stage 2

## Context

The parent plan commits to DeBruijn indices in canonical form; names are sidecar. For single-binder constructs (`lam`, `let`) the convention is unambiguous: the introduced name is `(var 0)` in the body, and any deeper reference shifts up by one.

For `rec` (arity 1+N per [ADR 0004](0004-rec-arity.md)) and `module` (arity N), N names are introduced *simultaneously* — they are all in scope in every binding RHS and (for `rec`) in the body. There is no "innermost" or "most recently declared" binder among them, so the index-to-binding mapping is a free choice that must be pinned for canonical determinism.

Two natural conventions:

1. **Position-K = index K.** Binding 0 is `(var 0)`, binding 1 is `(var 1)`, etc.
2. **Position-K = index N-1-K.** The last-listed binding is `(var 0)`, matching the let-cascade analogy where the most recently declared name in nested `let`s is `(var 0)`.

## Decision

**Position-K in the binding list = DeBruijn index K.** The first binding is `(var 0)`, the second is `(var 1)`, etc., both inside any binding RHS and inside the body.

## Alternatives considered

- **Position-K = index N-1-K (let-cascade analogy).** Defensible — it matches the intuition that nested `let`s and the rec-as-sequence-of-lets desugaring would produce. Rejected because (a) `rec` is not nested `let`s; the bindings are simultaneous, so the analogy doesn't actually hold, and (b) the simpler "position = index" rule is easier to explain, document, debug, and implement, with no semantic tradeoff.
- **Sort bindings by hash before assigning indices.** Would give canonical-form invariance under user permutation of binding order. Rejected because (a) sorting by hash requires resolving the indices first to compute the hashes — a fixpoint problem, and (b) this is best implemented as a separate normalizer pass, not as a canonical-form rule. Two `rec` groups with the same bindings in different orders are intentionally distinct in canonical form; users who want order-insensitive identity run a normalizer.
- **Defer the convention to Phase 1.** Tempting since Phase 0 has no canonicalizer implementation to bind by. Rejected because Stage 2's exit criterion is "two independent implementations produce byte-identical output" — both implementations need the same convention written down, even if the implementations themselves are post-Phase-0.

## Consequences

- **Inside any RHS of `(rec E0 E1 … BODY)`, `(var 0)` refers to E0.** Inside BODY, `(var 0)` also refers to E0. This is true for `module` too.
- **The let-cascade intuition does not apply to `rec`.** Documentation, tutorials, and the inspection view need to make this clear, or readers will mis-trace variable references. The inspection view typically shows display names, so the indexing convention is mostly invisible to users — but anyone debugging canonical text directly needs the rule.
- **DeBruijn under nested binders inside a `rec` RHS works as usual.** A `lam` inside binding K shifts the depth by 1; references to outer-rec bindings from inside a nested `lam` use indices ≥ K+1 (depending on how many intervening binders). The canonical-text-format spec § 11 includes a worked example.
- **Pattern-variable indexing inside `arm`s** is a separate question and is documented in [canonical-text-format.md § 4](../plans/canonical-text-format.md#binders), not here. The pattern-binding rule (last `pat-var` encountered = index 0) does follow the let-cascade analogy because patterns *are* sequentially encountered, unlike rec bindings.
- **Canonicalizer implementations must include a small test for this case.** The Stage 2 round-trip vectors include at least one mutual recursion fixture that exercises the convention.

## Related decisions

- [ADR 0004](0004-rec-arity.md) — `rec` is 1+N inner, `module` is N at top-level. This ADR specifies the index assignment for both.
- [ADR 0005](0005-canonical-surface-form.md) — surface form that hosts these indices.
