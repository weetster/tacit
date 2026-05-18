# 0093 - Bounded-stack `@loop` primitive

**Status:** Accepted
**Date:** 2026-05-18
**Phase:** Stateful host-bridge, Stage 2
**Closes:** [stateful-host-bridge-plan.md Stage 2](../plans/stateful-host-bridge-plan.md), Q-SHB-1
**Affirms:** [ADR 0027](0027-phase-1-rec-lowering.md),
[ADR 0073](0073-p4-function-values-and-closures.md),
[ADR 0074](0074-p4-higher-order-combinators.md)
**Amends:** [ADR 0041](0041-p2-structured-error-format.md) additively
(new diagnostic kinds)

## Context

Stage 2 of the stateful host-bridge track needs a reliable execution
primitive for long-running emulator-style loops. The current implementation
lowers every `rec` member as a top-level direct-call helper (ADR 0027), so a
self-recursive call pushes a stack frame per iteration. CPU-step loops over
millions of ticks blow the stack, and any apparent optimization is "optimizer
luck" the plan explicitly rules out.

Two design options were considered:

1. **Guaranteed self-tail-call lowering of `rec`.** Detect tail positions in
   `rec` bodies and apply LLVM `musttail` to direct self-calls. No new
   surface, but bounded-stack becomes an invisible codegen contract that
   depends on tail-position analysis through `if`/`match`/`let`. A subtle
   non-tail expression (e.g. arithmetic after the recursive call) silently
   loses the guarantee.
2. **A standalone iteration primitive.** Add a dedicated form for
   bounded-stack iteration. `rec` keeps current semantics (recursion may
   grow stack).

Tacit favors one canonical way to accomplish each goal so that best practices
are consistent and require less inference. Treating `rec` and a sugar layer
as two interchangeable ways to express a loop conflicts with that
principle and embeds a hidden codegen contract in general code. The standalone
primitive avoids both problems: it gives iteration its own visible surface,
and the codegen path is structural rather than pattern-recognized.

No design, implementation, or validation work for this stage may read, list,
search, or otherwise depend on `corpus/sealed/`.

## Decision

Add three compiler-recognized `@name` primitives following the
[ADR 0074](0074-p4-higher-order-combinators.md) combinator pattern: no new
canonical-text node, no parser change, no AST variant. The surface is an
`app` left-spine with a `sym` head; recognition lives in typecheck and
codegen primitive tables.

### Surface

```tacit
@loop init step
@loop-step value
@loop-exit value
```

- `init : S` — initial loop state.
- `step : S -> { tag : Int, value : S } / e` — callback producing the next
  iteration's directive.
- `@loop-step value` constructs `{ tag = 0, value }`; the loop continues with
  `value` as the next state.
- `@loop-exit value` constructs `{ tag = 1, value }`; the loop terminates with
  `value` as its result.

`@loop-step` and `@loop-exit` are ordinary expression-position primitives. They are not
control-flow keywords; they evaluate to a two-field record literal. There is
no positional restriction on where they may appear, but they are only useful
as the tail value of an `@loop` callback because that is the only context
that consumes the `{ tag, value }` record shape.

### State-type constraint

Loop state may be `Int`, any `FixedInt`, or a `Record` whose fields are
themselves loop-state-eligible. Borrowed vectors, `Buf`, `I64Vec`, `Vec`, and
function values are rejected as loop state through the same escape-check
that already governs closure captures (ADR 0073). The diagnostic points at
the `@loop` callback's argument position.

For Stage 2 the loop result type equals the loop state type (`R = S`). This
matches the CPU/PPU/APU step-loop shape ("iterate state, return final state")
and avoids needing union types. Future stages may relax `R ≠ S` once package
instances (Stage 3) provide heap state that does not need to round-trip
through the closure return value.

### Type and effect rules

- `@loop : forall S. S -> (S -> { tag : Int, value : S } / e) -> S / (e ∪ {Div})`
- `@loop-step : forall S. S -> { tag : Int, value : S }`
- `@loop-exit : forall S. S -> { tag : Int, value : S }`

The `Div` augmentation matches the multi-binding `rec` rule already in
inference: a loop may not terminate, so its evaluation effect includes
`Div`. Effects from `init`, callback evaluation, and callback call are
joined into the loop's overall effect.

### Codegen

`@loop` lowers as a labeled basic-block loop, mirroring the existing
`@map`/`@fold`/`@for-each` patterns. No `call` instruction iterates; the
back-edge is an LLVM `br` to the loop header. The step callback is invoked
once per iteration through the existing closure ABI; its stack frame is
reclaimed on return.

```
entry:    state₀ = init                    ; br header
header:   state = phi [state₀ from entry,
                       state' from cont]
          rec = call step(state)
          tag = extractvalue rec, 0
          switch tag → cont, exit
cont:     state' = extractvalue rec, 1     ; br header
exit:     loop_value = extractvalue rec, 1
```

`@loop-step` and `@loop-exit` lower to inline `insertvalue` sequences building the
two-field record literal; they are independent primitives that work outside
`@loop` as well, but only useful there.

### Diagnostics

The following structured diagnostic kinds are added:

| Kind | Meaning |
| --- | --- |
| `loop-state-shape-invalid` | `@loop` state type is a non-escapable handle (Buf, I64Vec, Vec, Fn) |
| `loop-callback-shape-invalid` | `@loop` callback is not `S -> { tag : Int, value : S }` |

Existing `type-mismatch` covers ordinary scalar/record arg mismatches.

## Relationship to `rec`

`rec` retains its current semantics unchanged: a recursion construct that may
grow stack. Use `rec` for tree-recursive helpers (parsers, AST walkers).
Use `@loop` for iteration with state. There is no tail-call analysis applied
to `rec`; no implicit codegen contract is added to existing programs.

## Alternatives considered

### Sugar for self-tail-recursive `rec`

Rejected. Adding `@loop` as sugar that desugars into a tail-recursive `rec`
plus codegen recognition of tail positions would create two canonical ways to
express iteration — the sugar and the hand-written `rec` form. Tacit's
one-canonical-way principle (see [[feedback_one_canonical_way]]) rules this
out. It would also bake a hidden contract ("this `rec` shape is special")
into general code, hurting LLM reasoning about which programs are bounded.

### `musttail` on `rec` without a new surface

Rejected. Same hidden-contract problem as the sugar option, with no visible
anchor to grep for or diagnose against. Tail-position analysis through
`if`/`match`/`let` would silently lose the guarantee on subtle bodies (e.g.
`@i64-add 1 (step ...)`).

### Predicate + step + result callbacks

Considered: `@loop init done? step result` with three pure callbacks. Clean
typing but no visible Step/Exit signal — termination is implicit in the
predicate, which loses the "this iteration is the last one" anchor.

### Tagged variant via `Ctor`

Considered: have `@loop-step`/`@loop-exit` construct `(ctor Step value)` /
`(ctor Exit value)` user-data variants. Codegen does not currently support
`Ctor` in expression position (ADR 0072 left value-position constructors
out of Phase 4). Reusing records `{ tag, value }` keeps the change within
already-supported value types.

### Group-wide bounded stack for `rec` groups

Rejected. Guaranteed bounded stack across mutually recursive `rec` members
would require either signature unification (padding params) or a dispatch
trampoline. Real emulator loops are self-recursive; the Stage 5 Tacboy slice
does not need mutual tail calls.

### New canonical AST node

Rejected. `(app (app (sym loop) init) step)` already expresses the surface.
ADR 0074 set this precedent for `@map`/`@fold`/`@for-each`; reusing it keeps
the canonical-text format frozen.

## Consequences

- Tacit gains a visible, structural iteration primitive that LLMs and humans
  can grep for and reason about without analyzing tail positions.
- Tight loops over millions of iterations compile to bounded-stack native code
  by construction (basic-block back-edge, not function call).
- `rec` semantics are unchanged; existing recursive programs are unaffected.
- The CPU/PPU/APU step loops needed for the Stage 5 Tacboy vertical slice
  have a guaranteed primitive to target.
- Future relaxation of `R = S` is a Stage 3+ concern once package instances
  provide heap-backed state.
- The canonical-text format remains frozen; no new tags.

## Related decisions

- [ADR 0027](0027-phase-1-rec-lowering.md) — `rec` lowering as
  forward-declared direct calls; loop here bypasses that entirely.
- [ADR 0073](0073-p4-function-values-and-closures.md) — closure ABI used to
  call the step callback.
- [ADR 0074](0074-p4-higher-order-combinators.md) — combinator surface
  precedent (`@map`/`@fold`/`@for-each`).
- [ADR 0091](0091-stateful-host-bridge-scope.md) — track scope.
- [ADR 0092](0092-rich-boundary-library-codegen.md) — Stage 1 boundary
  codegen.
