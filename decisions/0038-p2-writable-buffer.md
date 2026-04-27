# 0038 — Phase 2 writable-buffer binding model

**Status:** Accepted
**Date:** 2026-04-27
**Phase:** 2, Stage 1
**Closes:** [phase-2-plan.md Q-P2-5](../plans/phase-2-plan.md); [ADR 0032 § 3](0032-stage-4-frozen.md) smoke #8 deferral

## Context

[ADR 0032 § 3](0032-stage-4-frozen.md) deferred smoke #8 (`echo.tac`)
because it requires reading bytes from stdin into a writable buffer — a
capability absent from Phase 1's model. Phase 1's libc surface includes
`read(fd, buf, count)` ([ADR 0025](0025-phase-1-libc-surface.md)) but
provides no way to name or allocate the destination buffer in Tacit source.

`echo.tac` needs:
1. A buffer (mutable, stack-allocated, byte-addressable).
2. `@read 0 buf count` that fills the buffer.
3. `@write 1 buf count` that sends the buffer bytes to stdout.

The challenge: Tacit's canonical form is a functional AST. Mutable state
is an effect (`Mut` in the lattice from [ADR 0035](0035-p2-effect-set-canonical.md)),
not a structural feature. The buffer must be expressible in canonical form
without introducing a new ambient mutable namespace, while fitting inside
Phase 1's closed-lambda, let-binding discipline ([ADR 0026](0026-phase-1-closed-lambdas.md)).

Three constraints bound the design:
1. **No new canonical node kinds.** Existing nodes (`let`, `app`, `sym`,
   `int`) are sufficient if the buffer is modeled as a value bound by
   `let`. The alternative (a `buf-alloc` node kind) would extend the
   canonical format, which requires an amendment ADR. This ADR avoids
   that extension by using `@name` + `sym` in the established primitive
   pattern ([ADR 0028](0028-phase-1-libc-call-surface.md)).
2. **Phase 2 scope: stack allocation only, compile-time constant size.**
   Heap-allocated buffers with dynamic sizes require the ownership
   and lifetime system that is Tacit-Full scope. Stack allocation
   (`alloca` in LLVM) with a compile-time-known size is the minimum
   for Phase 2.
3. **Buffer lifetime = enclosing `let` scope; no escape.**
   The buffer handle must not outlive the `let` binding that creates it.
   The typechecker enforces this in Stage 2.

## Decision

**Writable buffers are modeled as a new primitive `@buf-alloc` that is
bound via `let`. The buffer handle is passed as an argument to `@read`
and `@write`. No new canonical node kinds are needed.**

### `@buf-alloc`: new primitive

`@buf-alloc` is added to the Phase 1 `@name` primitive allowlist. It is a
new category alongside LIBC, ARITH, and CMP:

| Category     | Symbol       | Arity | Return | Effect     | LLVM lowering        |
|--------------|--------------|-------|--------|------------|----------------------|
| `STACK-ALLOC`| `buf-alloc`  | 1     | buf    | `{Alloc}`  | `alloca [N x i8]*`   |

The single argument is the buffer size as a compile-time constant integer
(`int` expression). Phase 2 requires the size argument to reduce to an
integer literal after constant folding; dynamic sizes produce a
`codegen-error` in Phase 2 and are deferred to Phase 3+.

Return value: a buffer handle, typed as `Buf N` where N is the size (see
the type section below).

### Usage pattern

Canonical form for `let buf = @buf-alloc 1024 in @read 0 buf 1024`:
```
(let
  (app (sym buf-alloc) (int 1024))
  (app (app (app (sym read) (int 0)) (var 0)) (int 1024)))
```

`(var 0)` in the body refers to the buffer handle bound by `let`.
`@write` uses the same handle:
```
(let
  (app (sym buf-alloc) (int 1024))
  (let
    (app (app (app (sym read) (int 0)) (var 0)) (int 1024))
    (app (app (app (sym write) (int 1)) (var 1)) (int 1024))))
```

Here `(var 1)` in the inner let's body refers to the buffer (the outer
`let` binding, now shifted past the inner `let`'s binder for the
read-return value).

### `@read` and `@write` with buffer handles

When the second argument to `@read` or `@write` is a `buf-alloc`-typed
handle, the effect checker assigns the following effects:

| Primitive | Effect with buf handle  | Rationale                            |
|-----------|-------------------------|--------------------------------------|
| `@read`   | `{IO, Mut}`             | Syscall (IO) + mutates buffer (Mut)  |
| `@write`  | `{IO}`                  | Syscall (IO); reads buffer, no Mut   |
| `@buf-alloc` | `{Alloc}`            | Allocates stack memory               |

Effect of a typical `echo` body:
- `@buf-alloc 1024` → `{Alloc}`
- `@read 0 buf 1024` → `{IO, Mut}`
- `@write 1 buf 1024` → `{IO}`
- Combined (join): `{Alloc, IO, Mut}`

### Type of a buffer handle

The type of a buffer returned by `@buf-alloc N` is `Buf N` in authoring
view and `(app (sym Buf) (int N))` in canonical type position. `Buf` is a
built-in type constructor introduced by this ADR. It is:
- Not a user-definable type constructor (built-in only).
- Parameterized by the buffer's static size (an integer). Phase 2
  supports only compile-time-constant sizes, so `(int N)` in type
  position is uniquely this use case.
- Not a first-class value type: a `Buf N` value cannot be stored in a
  record, passed as a generic `a`, or returned from a function (the
  typechecker rejects these; scope-escape is a typecheck error).

`@read`, `@write` accept `Buf N` in their second argument position.
The typechecker validates that the count argument does not exceed N.
In Phase 2, this check is a static comparison of integer literals;
dynamic bounds checking is Phase 3+.

### Buffer lifetime enforcement

The typechecker enforces that a `Buf N` handle bound by `(let ... body)`
does not appear as a free variable in any expression returned from `body`
or stored in a data structure outside `body`. Concretely: the type `Buf N`
is a *linear* or *region-limited* type — it may be used within its `let`
scope but not captured in a closure or returned. In Phase 2, the
enforcement is simple: the typechecker flags any position where a `Buf N`
appears in a type that escapes the `let` binder.

No new type-system machinery is required for this in Phase 2 because Phase 2
already bans first-class function values ([ADR 0026](0026-phase-1-closed-lambdas.md))
and closures with free variables. The buffer anti-escape rule is a single
additional check in the typechecker.

### `libc-effects.toml` for `read` and `write`

The existing entries for `read` and `write` in `stdlib/libc-effects.toml`
list `tacit_effect_set = ["IO"]`. This is the base effect. When the second
argument is a buffer handle, the effect checker augments `read`'s effect
with `Mut` (because `read` writes into the buffer). This augmentation is
applied by the typechecker based on argument type analysis, not by changing
`libc-effects.toml`. The TOML file's schema ([ADR 0025](0025-phase-1-libc-surface.md))
is unchanged.

### Test vector shipped with this ADR

**V33 — buffer allocation and read** (`33-buf-alloc-read.canonical`):
```
(let (app (sym buf-alloc) (int 256)) (app (app (app (sym read) (int 0)) (var 0)) (int 256)))
```
Represents `let buf = @buf-alloc 256 in @read 0 buf 256`. Uses only
existing canonical node kinds (`let`, `app`, `sym`, `int`, `var`) and is
parseable by the Phase 1 canonical parser without modification.

## Alternatives considered

- **New canonical node kind `buf-alloc size body`.** A scoping form
  analogous to `let` that makes the lifetime boundary explicit in the AST:
  `(buf-alloc 1024 (app (app (app (sym read) (int 0)) (var 0)) (int 1024)))`.
  Cleaner semantics, but adds a new canonical tag and requires a canonical-
  format amendment. The `let`-based model achieves the same scoping with
  existing tags. Rejected: extra canonical complexity for no structural gain.

- **Heap-allocated buffers with dynamic sizes.** The `Alloc` effect is
  already in the lattice; heap allocation is semantically clean. But it
  requires an ownership/lifetime system to prevent use-after-free. Phase 2
  has no such system; stack allocation with a lifetime tied to the `let`
  scope avoids the problem entirely. Heap buffers are Phase 3+ when
  ownership semantics are designed. Rejected for Phase 2.

- **Pass buffers as integers (the buffer address).** Could re-use `@read`
  with an integer address and bypass type tracking. Rejected: this would
  make the effect checker unaware that `@read` is mutating anything, since
  it sees an `Int` argument rather than a `Buf N`. The `Mut` effect would
  be lost. The design commitment is that the typechecker is the source of
  truth for effects.

- **Represent a buffer as a `record` of bytes.** Functional and immutable:
  each `@read` returns a new record-of-bytes. Rejected: astronomically
  inefficient, and the interaction with `@write` requires converting the
  record back to a contiguous byte sequence. Tacit-Lite targets real
  system programs; this approach would make `echo.tac` unusable.

- **Augment `libc-effects.toml` with argument-position effect annotations.**
  Add a `arg_effects` table to the TOML schema so `read`'s second argument
  position carries `{Mut}`. This changes the TOML schema (frozen by
  ADR 0025) and adds complexity to the effect checker (it must inspect
  argument positions). The typechecker's argument-type analysis (is arg 1
  a `Buf N`?) is simpler and does not require touching the frozen schema.
  Rejected.

## Consequences

- Smoke #8 (`echo.tac`) is unblocked. Stage 4 of Phase 2 adds it to the
  corpus and CI.
- `buf-alloc` joins the STACK-ALLOC category in the `@name` primitive
  allowlist, extending the Phase 1 primitive table (ADR 0028) without
  changing existing LIBC, ARITH, or CMP categories.
- The typechecker gains a `Buf N` built-in type and three type-aware
  effect rules for `buf-alloc`, `read`, and `write`.
- Codegen in Stage 4 adds LLVM `alloca [N x i8]` emission for
  `buf-alloc`. The `alloca` pointer is passed directly to `read`/`write`
  as the second argument; LLVM handles the `i8*` cast.
- V33 round-trips on the Phase 1 canonical parser without any changes.
- The `libc-effects.toml` schema is unchanged; its three entries are
  unchanged. The `Mut` augmentation for `read` is typechecker logic,
  not TOML data.
- Buffer anti-escape is a Phase 2 typecheck-only policy. Phase 3+ may
  relax or formalize it as a linear-type or region-type feature.

## Related decisions

- [ADR 0025](0025-phase-1-libc-surface.md) — `libc-effects.toml` schema;
  unchanged by this ADR.
- [ADR 0026](0026-phase-1-closed-lambdas.md) — closed-lambda discipline;
  the buffer anti-escape rule is consistent with the existing no-free-
  variable restriction.
- [ADR 0028](0028-phase-1-libc-call-surface.md) — `@name` primitive
  allowlist; extended with STACK-ALLOC category.
- [ADR 0032 § 3](0032-stage-4-frozen.md) — smoke #8 deferral closed by
  this ADR's model definition and Stage 4's implementation.
- [ADR 0035](0035-p2-effect-set-canonical.md) — `Alloc`, `IO`, `Mut`
  effect atoms; all three appear in `echo.tac`'s effect set.
- [phase-2-plan.md Q-P2-5](../plans/phase-2-plan.md) — closed.
