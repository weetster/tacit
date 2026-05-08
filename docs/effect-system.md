# Effect Systems in Tacit

Tacit tracks side effects in the type system. This document explains the terminology used in the project plan — what "simple" vs. "advanced" effects mean, what features live in Tacit-Lite vs. Tacit-Full, and why the boundary is drawn where it is.

## What an "effect" is, as a type system concept

Normally, a function's type tells you about values: `sort :: [int] → [int]`. An effect system extends the type to also tell you *what the function does to the world* beyond producing a return value. A pure function's effect set is empty. A function that reads a file has `IO`. A function that mutates shared state has `Mut`. The effect set is part of the signature, visible at the call site, and enforced by the compiler.

Notation varies across languages. Koka uses `sort :: [int] → [int]` for pure and `log :: str → () / IO` for effectful (the slash separates value type from effect set). Haskell uses monads: `IO ()` wraps the return instead. Tacit will pick one; for discussion here, we use the Koka-style slash.

## The simple (Lite) version

### Fixed lattice

"Lattice" is math jargon for a partial order with union and intersection operations. The effect lattice is the space of possible effect sets. In Lite, the set of *atomic* effects is fixed at four:

- **`IO`** — touches the outside world (files, network, stdin/stdout)
- **`Alloc`** — allocates on the heap
- **`Mut`** — mutates existing state someone else can observe
- **`Div`** — might not terminate (diverge)

Every function's effect set is a subset of those four. Users can't add new atoms. This keeps the system decidable and simple, at the cost of expressiveness.

Note that there is no `Exn` atom: failure is expressed as `Result` types in the return value, not as a side effect. Panic aborts the process and is not normal control flow. See [tacit-plan.md § Decisions baked in](../plans/tacit-plan.md).

### Effect inference

The compiler figures out a function's effect set by walking its body: union of all callees' effects, plus any primitive effect-producing operations. You don't annotate most of the time; annotations are only needed when the inferred set is wider than you want and the compiler flags it.

### Basic effect polymorphism

This is the critical piece for higher-order functions. If `map` required its callback to be pure, you couldn't map a logging function over a list. The solution: an effect *variable*.

```
map :: (a → b / e) → [a] → [b] / e
```

Here `e` is a variable standing for whatever effect set the callback has. Call `map` with a pure callback, the result is pure. Call it with an IO callback, the result is IO. One variable per function is "basic polymorphism" — enough for standard combinators (map, filter, fold) but not for composing multiple effectful operations in complex ways.

### Phase 4 function values and combinators

Phase 4 implements the Lite version of higher-order effects without crossing
into row polymorphism. A function value carries a `fn-ty` call effect. Calling
that function contributes the recorded effect at the call site, whether the
function is a direct lambda, a capturing closure, a returned function value, or
a callback stored in a record.

The Phase 4 combinators use this existing mechanism:

- `@map xs count f out` calls `f : Int -> Int / e`; the combinator has the
  callback effect `e` plus `Mut` because it writes the output `I64Vec`.
- `@fold xs count init f` calls `f : Int -> Int -> Int / e` with the
  accumulator first and element second. The first curried application must be
  pure; the final element-consuming application carries `e`.
- `@for-each xs count f` calls `f : Int -> Int / e`, ignores the integer
  result, and has callback effect `e`.

Closure environment allocation is compiler-managed and is not exposed as a
source-level `Alloc` effect. `Buf` and `I64Vec` handles are non-escapable and
cannot be captured by first-class closures; that restriction keeps the Lite
effect story inside the fixed lattice.

## What "advanced" adds (Full)

### Effect handlers

Handlers are to effects what `try/catch` is to exceptions, except they work for any effect and can *resume* execution with a replacement result. Example:

```
handle (print "hello"; print "world") with
  IO.print(msg) → log_to_memory(msg); resume ()
```

Inside the handler, the `print` calls never actually hit stdout — they're intercepted and redirected to an in-memory log. The handler can resume the computation as if the print had happened normally. This makes handlers essentially *delimited continuations* in disguise, which is why they're powerful and why they took Koka years to get right. You can use them to implement exceptions, generators, async/await, dependency injection, and testable IO, all from the same mechanism.

### User-defined effects

Instead of a fixed lattice, Full lets you declare new effect kinds:

```
effect State<s> {
  get :: () → s
  put :: s → ()
}
```

Now any function that uses `get`/`put` has the `State<s>` effect, and you can write a handler that implements it (threaded as a tuple, stored in a ref cell, serialized to disk — whichever). The effect becomes a protocol, not a hardcoded capability. This is where effect systems become genuinely expressive, and also where they become a whole research agenda.

### Row polymorphism

Basic polymorphism has a single effect variable per function. Row polymorphism lets a signature talk about "these concrete effects, plus whatever else the row contains":

```
log_and_call :: (a → b / {e}) → a → b / {IO | e}
```

This says `log_and_call` adds `IO` to whatever effects its callback has, keeping them all. Without row polymorphism, composing multiple effectful functions forces awkward unifications. It's analogous to row polymorphism in record types: "this record has at least fields X and Y, and possibly others."

## Why we split along this seam

The simple version is a ~3-week addition to Phase 2. The advanced version is a multi-quarter research project. The boundary between them is also where decidability and performance stop being free. Simple effects can be checked in near-linear time; handlers require non-trivial type inference algorithms and runtime support (since they can capture continuations). Keeping Lite on the simple side lets us ship; keeping Full on the advanced side gives the research stretch room to breathe.

The reasoning payoff for AI is mostly already there at the simple level: "can I tell what this function touches without reading its body?" — yes, even with the fixed lattice. The advanced features pay off for specific use cases (testability, custom protocols) rather than for basic code comprehension.
