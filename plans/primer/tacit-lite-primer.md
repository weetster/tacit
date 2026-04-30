# Tacit-Lite Primer

## 1. Semantic Summary

Tacit-Lite is a small expression language whose durable identity is a
content-addressed AST. Humans and models write the authoring view: `let`,
`lambda`, `rec`, `if`, `match`, records, and `@name` primitive calls. The
compiler parses that view to the AST, typechecks it, emits LLVM IR, and can
render it back without losing authoring names through the sidecar.

A Tacit program is usually one expression. A binding extends only the body
after `in`. A lambda has exactly one parameter, so multi-argument functions
are curried: `lambda x. lambda y. ...`, then called as `f a b`. Recursive
helpers use `rec {name = lambda ...; ...} in body`. A `rec` group is the
only way a function can call itself or a sibling helper.

Type inference is local. Exported programs carry sidecar type/effect
metadata, but ordinary examples rely on inference. The base runtime values
are `Int`, `Bool`, `Str`, `Buf`, records, constructors, lambdas, and holes.
The effect lattice has four atoms: `Alloc`, `Mut`, `IO`, and `Div`. Pure code
has `{}`. Allocation of stack buffers adds `Alloc`; buffer writes and integer
formatting add `Mut`; `@read`, `@write`, and `@exit` add `IO`; recursive calls
and division-like primitives can add `Div`.

There is no implicit mutable state. Mutation is explicit through `Buf`
primitives. There is no general heap buffer, hash map, object system, type
class, effect handler, or user-defined effect in Phase 3. If a task wants
those, write the direct Tacit-Lite shape with the available primitives.

## 2. Progressive Python/Rust/Tacit Pairs

### Pair 1: Arithmetic Primitive

Python:

```python
def main() -> int:
    return 40 + 2
```

Rust:

```rust
fn main() -> i64 {
    40 + 2
}
```

Tacit:

```tacit
@add 40 2
```

Primitive calls are written with `@`. Applications are left-associative, so
`@add 40 2` means `((@add 40) 2)`.

### Pair 2: `let` Binds One Value

Python:

```python
def main() -> int:
    x = 40
    return x + 2
```

Rust:

```rust
fn main() -> i64 {
    let x = 40;
    x + 2
}
```

Tacit:

```tacit
let x = 40 in @add x 2
```

The right side of `let` is evaluated, bound to the name, and then visible
only in the body after `in`.

### Pair 3: Conditions

Python:

```python
def main() -> int:
    return 1 if 3 < 5 else 0
```

Rust:

```rust
fn main() -> i64 {
    if 3 < 5 { 1 } else { 0 }
}
```

Tacit:

```tacit
if @lt 3 5 then 1 else 0
```

`if` branches must have the same type. Conditions accept booleans and the
existing integer truth convention used by the compiler.

### Pair 4: Integer Pattern Match

Python:

```python
def main() -> int:
    n = 5
    if n == 5:
        return 1
    return 0
```

Rust:

```rust
fn main() -> i64 {
    match 5 {
        5 => 1,
        _ => 0,
    }
}
```

Tacit:

```tacit
match 5 with | 5 => 1 | _ => 0
```

Use `match` when the cases are data-shaped. For plain comparisons, `if` is
usually shorter.

### Pair 5: A Named Helper

Python:

```python
def main() -> int:
    def inc(x: int) -> int:
        return x + 1
    return inc(41)
```

Rust:

```rust
fn inc(x: i64) -> i64 {
    x + 1
}
```

Tacit:

```tacit
let inc = lambda x. @add x 1 in inc 41
```

Use a named helper when the computation has a clear role or more than one
line. Inline one-shot lambdas only when binding the name adds no clarity.

### Pair 6: Curried Multi-Argument Helper

Python:

```python
def add2(x: int, y: int) -> int:
    return x + y
```

Rust:

```rust
fn add2(x: i64, y: i64) -> i64 {
    x + y
}
```

Tacit:

```tacit
let add2 = lambda x. lambda y. @add x y in add2 40 2
```

Every lambda binds one parameter. Multi-argument functions are nested
lambdas and ordinary left-associated calls.

### Pair 7: Self Recursion

Python:

```python
def fact(n: int) -> int:
    if n == 0:
        return 1
    return n * fact(n - 1)
```

Rust:

```rust
fn fact(n: i64) -> i64 {
    if n == 0 { 1 } else { n * fact(n - 1) }
}
```

Tacit:

```tacit
rec {fact = lambda n. if n then @mul n (fact (@sub n 1)) else 1} in fact 5
```

A self-call must be inside `rec`. The recursive name is visible in its own
lambda body and in the body after the group.

### Pair 8: Mutual Recursion

Python:

```python
def even(n: int) -> int:
    return odd(n - 1) if n else 1

def odd(n: int) -> int:
    return even(n - 1) if n else 0
```

Rust:

```rust
fn even(n: i64) -> i64 {
    if n == 0 { 1 } else { odd(n - 1) }
}
```

Tacit:

```tacit
rec {even = lambda n. if n then odd (@sub n 1) else 1; odd = lambda n. if n then even (@sub n 1) else 0} in even 4
```

Put mutually recursive helpers in the same `rec` group. The whole group is
one recursive atom for identity and lowering.

### Pair 9: Buffer Read/Write

Python:

```python
def main() -> int:
    data = [40, 2]
    return data[0] + data[1]
```

Rust:

```rust
fn main() -> i64 {
    let data = [40_i64, 2_i64];
    data[0] + data[1]
}
```

Tacit:

```tacit
let buf = @buf-alloc 2 in
let _ = @buf-set buf 0 40 in
let _ = @buf-set buf 1 2 in
@add (@buf-get buf 0) (@buf-get buf 1)
```

`Buf` is the Phase 3 mutable byte/int buffer. Bind writes to `_` when only
the mutation matters.

### Pair 10: Parse Bytes as an Integer

Python:

```python
def main() -> int:
    return int("42")
```

Rust:

```rust
fn main() -> i64 {
    "42".parse::<i64>().unwrap()
}
```

Tacit:

```tacit
let buf = @buf-alloc 2 in
let _ = @buf-set buf 0 52 in
let _ = @buf-set buf 1 50 in
@parse-i64 buf 0 2
```

`@parse-i64 buf off len` parses ASCII digits in a buffer slice. It is pure:
the buffer is read but not mutated.

### Pair 11: Format and Write

Python:

```python
def main() -> int:
    print(42)
    return 0
```

Rust:

```rust
fn main() -> i64 {
    println!("42");
    0
}
```

Tacit:

```tacit
let out = @buf-alloc 32 in
let w = @fmt-i64 out 0 42 in
let _ = @write 1 out w in
0
```

`@fmt-i64` writes decimal bytes into a buffer and returns the byte count.
`@write 1 out w` writes those bytes to stdout.

### Pair 12: Read One Byte

Python:

```python
def main() -> int:
    data = input()
    return len(data[:1])
```

Rust:

```rust
fn main() -> i64 {
    let mut b = [0_u8; 1];
    std::io::stdin().read(&mut b).unwrap() as i64
}
```

Tacit:

```tacit
let buf = @buf-alloc 1 in
let n = @read 0 buf 1 in
n
```

`@read 0 buf 1` reads from stdin, mutates `buf`, and returns the number of
bytes read.

### Translation Routine

When translating a small Python or Rust solution to Tacit-Lite, do the work
in this order:

1. Identify the value that the whole program returns. In Phase 3 examples
   this is almost always an `Int`, often after printing bytes to stdout.
2. Pull named helper functions out before writing the main expression. A
   helper that calls itself or another helper in its group goes in `rec`; a
   helper that does not recurse is a `let`.
3. Replace loops with recursive helpers or with a fixed sequence of buffer
   operations when the size is static. Tacit-Lite has no `while` or `for`
   keyword in Phase 3.
4. Replace Python lists, bytearrays, and Rust arrays with `Buf` where the
   values are integer bytes or small integer slots. Use explicit offsets.
5. Replace standard-library parsing and formatting with `@parse-i64` and
   `@fmt-i64`. Do not hand-roll those unless the task is specifically about
   parsing or formatting internals.
6. Check the effect story last. If the program reads or writes, it has `IO`.
   If it allocates a buffer, it has `Alloc`. If it writes to a buffer, it has
   `Mut`. If it recurses or divides, it may have `Div`.

Tacit code is densest when it keeps the computation in expression form. A
long chain of `let`s is normal and readable. Avoid translating statement for
statement when a helper can express the loop state directly. For example, a
Python loop with `total`, `i`, and `n` usually becomes `rec {loop = lambda
state. ...}` where `state` is an encoded integer or one parameter per value,
depending on which shape is clearer.

### Reading Tacit Application

Application has no comma syntax. Read `f a b c` as `(((f a) b) c)`.
Parentheses are only needed when an argument is itself a compound
expression, as in `@mul n (fact (@sub n 1))`. If codegen reports an arity
problem, first check whether a missing argument left a primitive partially
applied.

Primitive names should remain primitive names. A wrapper like `let plus =
lambda x. lambda y. @add x y in ...` is useful only when it participates in a
larger abstraction. It is not useful as a synonym for `@add`.

### Choosing Between `if` And `match`

Use `if` when there is one condition and two outcomes. Use `match` when the
branches correspond to values or constructors. In the current corpus-shaped
programs, integer matching is useful for small sentinels, parser states, and
compact zero/non-zero cases where the branch names are clearer than nested
comparisons.

### Buffer Mental Model

Treat `Buf` as a capability-like handle scoped by `let`. A buffer is created
by `@buf-alloc` or `@buf-alloc-dyn`, then passed explicitly to every read or
write primitive. There is no implicit current buffer and no indexing sugar.
The index is always an integer argument. Because the buffer handle is not a
heap object in Phase 3, keep it inside the expression that owns it; do not
try to return a closure that stores a buffer for later use.

### Output Mental Model

Tacit output is explicit byte output. To print an integer, allocate a buffer,
format into it, write exactly the returned byte count, and optionally write a
newline string:

```text
let out = @buf-alloc 32 in
let w = @fmt-i64 out 0 value in
let _ = @write 1 out w in
let _ = @write 1 "\n" 1 in
0
```

The fenced block above is `text` because `value` is a placeholder. In real
Tacit, bind `value` first or inline the expression that computes it.

## 3. Idiom Catalog

Use `let name = value in body` for sequential computation. Long programs are
usually a chain of top-level `let`s. Keep the name close to the value it
describes.

```tacit
let left = @add 20 20 in
let right = 2 in
@add left right
```

Use `lambda x. body` for a single argument. For two arguments, nest lambdas;
do not invent tuple arguments.

```tacit
let sub_then_add = lambda x. lambda y. @add (@sub x y) y in sub_then_add 50 8
```

Use `rec` exactly when a helper calls itself or a sibling. Keep the group as
small as possible.

```tacit
rec {sum = lambda n. if n then @add n (sum (@sub n 1)) else 0} in sum 5
```

Use `if` for two branches selected by a comparison or truthy integer.

```tacit
let n = 9 in if @gt n 3 then @sub n 3 else 0
```

Use `match` when the shape is a pattern. Integer patterns and `_` are the
most common Phase 3 use.

```tacit
match 0 with | 0 => 42 | _ => 1
```

Use primitive calls directly. Do not alias `@add`, `@read`, or `@fmt-i64`
behind friendlier names unless the helper also adds real logic.

```tacit
@mul (@add 2 3) (@sub 10 4)
```

Use buffers with explicit size, offset, and length. The primitive names state
what mutates.

```tacit
let a = @buf-alloc 2 in
let b = @buf-alloc 2 in
let _ = @buf-set a 0 65 in
let _ = @buf-copy b 0 a 0 1 in
@buf-eq a 0 b 0 1
```

Use `@scan-byte` when a byte search is the job. A missing byte returns a
sentinel defined by the primitive contract.

```tacit
let buf = @buf-alloc 3 in
let _ = @buf-set buf 0 65 in
let _ = @buf-set buf 1 66 in
let _ = @buf-set buf 2 67 in
@scan-byte buf 0 3 66
```

Use dynamic allocation when the size is computed at runtime.

```tacit
let n = @add 2 3 in
let buf = @buf-alloc-dyn n in
let _ = @buf-set buf 0 7 in
@buf-get buf 0
```

Use annotations sparingly in authoring examples. A base annotation is written
as `(expr:@Type)`.

```tacit
(5:@Int)
```

When a recursive helper needs an outer value, close over it normally. Phase 3
lowers that closed value as a hidden direct-call parameter.

```tacit
let base = 40 in
rec {plus_base = lambda x. @add x base} in plus_base 2
```

### Names

Names are display metadata, but they matter for model fluency. Prefer names
that describe the local role: `n`, `i`, `len`, `buf`, `out`, `w`, `loop`,
`state`, `acc`, `head`, `tail`, `left`, `right`. Do not rename only to shave
tokens. The sidecar preserves the authoring names through a round trip, so a
model should write names it can reason about when it repairs the program.

### Local State Encoding

When a helper needs several small integer state values and Tacit syntax makes
multi-parameter recursion awkward for a tiny example, an encoded integer state
can be acceptable. The carry-over `sum-numbers` example uses a high/low split
to keep a running sum and current number in one integer. Use that sparingly:
it is compact, but it is harder to repair. If two nested lambdas are clearer,
prefer them:

```text
rec {loop = lambda i. lambda acc. if done then acc else loop next_i next_acc} in loop 0 0
```

This is `text` because `done`, `next_i`, and `next_acc` are placeholders.

### Records And Projection

Records are useful for pure grouping, but Phase 3 codegen does not target a
large record-heavy style. Use records when a function naturally returns a
small bundle that is consumed immediately. Field order in canonical form is
sorted; authoring order is display metadata. If you project a field, make
sure the inferred record type actually contains that field.

### Constructors And Patterns

Capitalized identifiers are constructors. `True` and `False` are known
nullary boolean constructors. Other constructors can appear in parsed syntax
and patterns, but Phase 3 corpus references mostly avoid algebraic data
construction at runtime because the codegen subset is intentionally small.
For integer-heavy tasks, prefer `match` with integer patterns or `if` with
comparison primitives.

### Primitive Surface

Arithmetic: `@add`, `@sub`, `@mul`, `@div`, `@mod`.

Comparison: `@eq`, `@ne`, `@lt`, `@le`, `@gt`, `@ge`.

IO: `@read`, `@write`, `@exit`.

Allocation: `@buf-alloc`, `@buf-alloc-dyn`.

Buffer mutation and inspection: `@buf-get`, `@buf-set`, `@buf-copy`,
`@buf-eq`, `@scan-byte`.

Parsing and formatting: `@parse-i64`, `@fmt-i64`.

The primitive call shape is part of the language contract. For example,
`@buf-copy dst dst_off src src_off len` mutates `dst` and returns an `Int`;
`@buf-eq a a_off b b_off len` is pure and returns an `Int` flag.

### Sidecar Boundary

The model-facing program is authoring-view Tacit. The sidecar is not part of
the generated text in the Phase 3 evaluation prompt, but checked-in examples
and references use sidecars to record top-level type/effect expectations. If
a file has:

```toml
[types.main]
type = "Int"
effects = ["Alloc", "IO", "Mut"]
```

then the source expression must infer `Int` with exactly those effects. If it
prints but the sidecar lists only `["Alloc", "Mut"]`, the checker reports an
effect violation.

## 4. Effect Reasoning

A pure program has no allocation, mutation, IO, or possible divergence beyond
ordinary finite evaluation.

```tacit
let square = lambda x. @mul x x in square 6
```

This example is pure because `@mul` is pure and calling `square` does not use
any effectful primitive. A sidecar entry for the top-level program would use
`effects = []`.

Allocation is an effect even if the program later returns an integer.

```tacit
let buf = @buf-alloc 1 in
let _ = @buf-set buf 0 42 in
@buf-get buf 0
```

This evaluates with `{Alloc, Mut}`: `@buf-alloc` allocates and `@buf-set`
mutates. `@buf-get` is pure.

IO joins with other effects. Reading into a buffer produces both `IO` and
`Mut`.

```tacit
let buf = @buf-alloc 1 in
let n = @read 0 buf 1 in
@add n (@buf-get buf 0)
```

The full set is `{Alloc, IO, Mut}`. The top-level sidecar for this kind of
program must include all three atoms.

Formatting an integer mutates the output buffer but does not itself perform
IO. Writing the formatted bytes performs IO.

```tacit
let out = @buf-alloc 32 in
let w = @fmt-i64 out 0 (@add 20 22) in
let _ = @write 1 out w in
0
```

The full set is `{Alloc, IO, Mut}`. If the `@write` line were removed, it
would be `{Alloc, Mut}`.

Division-like primitives carry the `Div` effect because they can fail or
diverge under bad operands.

```tacit
@div 84 2
```

A recursive helper carries `Div` through its call effect. The compiler does
not prove termination in Phase 3.

```tacit
rec {count = lambda n. if n then count (@sub n 1) else 0} in count 3
```

Effect errors usually mean the sidecar or annotation under-declared the
effect set. The fix is to include the inferred atoms, not to hide the
primitive call.

### Effect Join Rules

Effects join by union. If one part of a program is `{Alloc}` and a later part
is `{Mut}`, the whole expression has `{Alloc, Mut}`. The order shown in
sidecars is stable and alphabetic: `Alloc`, `Div`, `IO`, `Mut`.

Partial application matters. `@write 1` is just a pure function value waiting
for the buffer and length. The `IO` effect appears only at the fully applied
call. This is why helper construction can remain pure even if calling the
helper later performs IO.

`let` joins the effect of its right-hand side with the effect of its body.
`if` joins the condition, then branch, and else branch. `match` joins the
scrutinee and every arm body. A lambda expression itself is pure to create;
the effect is attached to the function call. A recursive function's call
effect includes `Div` because Phase 3 does not prove the recursion terminates.

### Common Effect Predictions

`@add 1 2`: `{}`.

`@div 10 2`: `{Div}`.

`let b = @buf-alloc 1 in 0`: `{Alloc}`.

`let b = @buf-alloc 1 in @buf-set b 0 7`: `{Alloc, Mut}`.

`let b = @buf-alloc 1 in @read 0 b 1`: `{Alloc, IO, Mut}`.

`let b = @buf-alloc 32 in @fmt-i64 b 0 42`: `{Alloc, Mut}`.

`let b = @buf-alloc 32 in let w = @fmt-i64 b 0 42 in @write 1 b w`:
`{Alloc, IO, Mut}`.

`rec {f = lambda n. if n then f (@sub n 1) else 0} in f 3`: `{Div}`.

### Repairing Effect Violations

When the checker says the inferred effect set is not a subset of the declared
set, do not delete useful work to satisfy the sidecar. The declared effects
are the boundary claim. The source decides the truth. Add missing effects to
the sidecar or to the annotation if the program is otherwise correct.

When an effect appears surprising, find the innermost primitive that creates
it. `Mut` usually comes from `@buf-set`, `@buf-copy`, `@fmt-i64`, or
`@read`. `IO` comes from `@read`, `@write`, or `@exit`. `Alloc` comes from a
buffer allocation primitive. `Div` comes from recursion, division, or modulo.

## 5. Negative Examples And Diagnostics

Each failing Tacit block is marked with the diagnostic kind the fixture
expects. The JSON shape is always the Phase 2 envelope:
`{"schema_version":"p2.0","errors":[...]}`.

Wrong base annotation:

```tacit fail=type-mismatch
(5:@Str)
```

Diagnostic kind: `type-mismatch`. Fix: change the annotation to `@Int` or
change the expression to a string.

```tacit
(5:@Int)
```

Arithmetic on a boolean:

```tacit fail=operator-overload-failure
@add (@eq 1 2) 5
```

Diagnostic kind: `operator-overload-failure`. Fix: keep arithmetic operands
as integers.

```tacit
@add 1 5
```

Unknown type name:

```tacit fail=unresolved-type
(1:@Foo)
```

Diagnostic kind: `unresolved-type`. Fix: use a known type such as `@Int`.

```tacit
(1:@Int)
```

Unbound lowercase expression name:

```tacit fail=unbound-name
foo
```

Diagnostic kind: `unbound-name`. Fix: bind the name before using it.

```tacit
let foo = 1 in foo
```

Missing expression:

```tacit fail=expected-expr
_
```

Diagnostic kind: `expected-expr`. Fix: replace the hole with an expression.

```tacit
0
```

Unexpected token after a lambda binder:

```tacit fail=unexpected-token
lambda x. => x
```

Diagnostic kind: `unexpected-token`. Fix: put the body expression after the
dot.

```tacit
let id = lambda x. x in id 0
```

Buffer primitive called with an integer where a buffer is required:

```tacit fail=type-mismatch
@buf-get 0 0
```

Diagnostic kind: `type-mismatch`. Fix: allocate or receive a `Buf`.

```tacit
let buf = @buf-alloc 1 in @buf-get buf 0
```

String condition:

```tacit fail=type-mismatch
if "x" then 1 else 0
```

Diagnostic kind: `type-mismatch`. Fix: use a comparison or integer flag as
the condition.

```tacit
if @eq 1 1 then 1 else 0
```

String passed to an arithmetic primitive:

```tacit fail=operator-overload-failure
@add "x" 1
```

Diagnostic kind: `operator-overload-failure`. Fix: parse or compute an
integer first.

```tacit
@add 41 1
```

Unknown primitive-like symbol:

```tacit fail=unresolved-type
@frobnicate 1
```

Diagnostic kind: `unresolved-type`. Fix: use a primitive from the Phase 3
surface, or bind a lowercase helper name before using it.

```tacit
@add 1 0
```

### Diagnostic Envelope Pattern

The checker emits a JSON object with schema version `p2.0`. A successful JSON
check emits an empty error list. A failing check emits one or more entries:

```json
{
  "schema_version": "p2.0",
  "errors": [
    {
      "kind": "type-mismatch",
      "severity": "error",
      "location": {"ast_path": [{"child": 0}], "source_span": null},
      "message": "type mismatch: expected Int, got Str",
      "expected": {"sym": "Int"},
      "actual": {"sym": "Str"},
      "fix": null,
      "related": []
    }
  ]
}
```

The exact `location.ast_path` depends on where the error appears in the AST.
The important repair signals are `kind`, `message`, `expected`, and `actual`.
For model repair, read the first error, fix the smallest local expression that
can cause it, then rerun the checker. Later errors may disappear after the
first hole or type mismatch is fixed.

### Negative Example Reading Order

For `type-mismatch`, inspect the nearest argument, annotation, branch, field,
or primitive call. The checker reports the type it expected and the type it
inferred. If either side is `Unknown`, a previous diagnostic probably hid the
real source.

For `operator-overload-failure`, keep the operator and fix the operands.
Arithmetic operators want integer operands in Phase 3. Comparison operators
also compare integers and return booleans.

For parser-recovery diagnostics such as `unexpected-token` and
`expected-expr`, repair syntax first. A hole has an unknown type, so a single
syntax hole can create misleading downstream messages.

For `unresolved-type`, distinguish a type annotation problem from an unknown
symbol problem. In authoring view, `@Foo` inside a type annotation is treated
as a type symbol. In expression position, an unknown `@foo` primitive-like
symbol is also reported through the unresolved-type path because primitives
live in the typechecker surface.

## 6. Compiler Error Catalog

`type-mismatch`: an inferred type is not compatible with an expected type.
Usually fix the argument, branch, annotation, or primitive call site.

`operator-overload-failure`: an arithmetic or comparison operator received
operands that cannot share the required numeric type. Keep both operands as
integers in Phase 3.

`unresolved-type`: a type name, constructor, field projection, or unknown
primitive-like symbol is not known to the typechecker. Use a known base type,
field, constructor, or primitive.

`unbound-type-variable`: a `ty-var` appears without an enclosing `forall`.
In authoring-view examples, avoid raw type variables unless the surrounding
type expression binds them.

`type-arity-mismatch`: a type constructor was applied to the wrong number of
arguments. Phase 3 references are mostly monomorphic; avoid unnecessary type
application.

`module-missing-annotation`: an exported module binding lacks a type/effect
signature. This is a warning. Add the sidecar `[types.<binding>]` entry for
programs that ship as files.

`effect-violation`: an inferred effect set is not a subset of the declared
effect set. Add the missing atoms, commonly `Alloc`, `Mut`, `IO`, or `Div`.

`unbound-effect-variable`: an `eff-var` appears without an enclosing
effect-polymorphic `forall`. Phase 3 references should use concrete effect
sets instead.

`buf-escape`: a buffer handle is used outside the scope where the checker can
prove it is valid. Keep buffer use inside the `let` body that owns it.

`parse-error`, `unexpected-token`, `expected-expr`, `unclosed-paren`,
`expected-pattern`, `unbound-name`, and `arity-mismatch` come from parser
recovery holes. The parser still produces an AST, but the hole flows through
typechecking as an error. Fix the local syntax first; later diagnostics may
be consequences of the hole.

`test-failure` is introduced by the Phase 3 metrics file, not by the
typechecker. It means a generated program compiled and ran but failed one or
more corpus tests. Fix the algorithm, not the type signature.

### Repair Checklist

When a generated program fails, use this sequence:

1. Parse error or hole diagnostic: fix the local authoring syntax.
2. Unknown name or primitive: bind the lowercase name, capitalize a real
   constructor, or use a primitive from the Phase 3 surface.
3. Type mismatch at a primitive: check argument order. Buffer primitives are
   strict about `buf`, `offset`, `length`, and `value` positions.
4. Branch mismatch: make both branches return the same type.
5. Effect violation: update the boundary effect set to match source behavior.
6. Codegen unsupported: rewrite toward the Phase 3 subset: integer result,
   closed lambdas, `rec`, `if`, `match`, `let`, and supported primitives.
7. Test failure after compile success: keep the type/effect shape and debug
   the algorithm with smaller input.

### Common Task Recipes

These recipes are intentionally language-level, not corpus answers. They name
the shapes that appear across small programming tasks without copying any
task reference.

Summation over a known range: write a recursive helper with an index and an
accumulator. The condition checks whether the index has reached the bound.
The recursive call advances the index and accumulator. If the bound is known
at authoring time and tiny, a fixed chain of `@add` calls is sometimes
clearer, but for task-shaped input use recursion.

Counting bytes in stdin: allocate a one-byte buffer, read one byte at a time,
and carry the count in the recursive state. If `@read` returns zero, return
the count. If a byte should be skipped, recurse with the old count; otherwise
recurse with `@add count 1`. This shape has `{Alloc, IO, Mut, Div}` because
it allocates, reads, mutates the buffer, and recurses.

Parsing newline-separated integers: keep two integer states, the running
total and the current number. On digit bytes, update the current number with
`current * 10 + digit`. On newline, add current to total and reset current.
On EOF, return `total + current`. If carrying two lambda parameters makes the
source clearer, use `lambda total. lambda current. ...`; if a very compact
program is needed, an encoded state is possible but harder to repair.

Formatting several integers: reuse one output buffer. For each integer, call
`@fmt-i64 out 0 value`, write `out` for the returned length, then write a
newline string if the required output is line-oriented. Each `@fmt-i64`
mutates; each `@write` performs IO.

Searching a buffer: prefer `@scan-byte` for a byte target and `@buf-eq` for a
slice equality check. Hand-written recursive scans are acceptable when the
predicate is richer than equality, but they are longer and more error-prone.
Keep offsets and lengths explicit.

Copying a slice: use `@buf-copy dst dst_off src src_off len`. Remember that
the first buffer is the destination. Many wrong answers reverse `dst` and
`src`; the typechecker cannot catch that because both are `Buf`.

Sorting a fixed tiny buffer: direct buffer reads and writes can be clearer
than a general recursive sort. For a dynamic or task-shaped size, write a
recursive outer loop and inner loop, or choose a simple algorithm whose state
fits cleanly in integers and buffers. Prefer correctness over clever token
packing.

Comparing flags: comparison primitives return `Bool`, while many older smoke
examples use integer truthiness. Both work in conditions, but arithmetic
expects integers. If you need to add a flag, convert through an `if`: return
`1` for true and `0` for false.

### Input/Output Discipline

Tacit-Lite has no implicit command-line argument parser. Corpus-style
programs receive input on stdin and write bytes to stdout. A generated
program should not assume environment variables, files by path, or a process
argument vector unless a future stage adds that surface. For Phase 3, the
portable pattern is:

```text
read stdin bytes -> parse or scan -> compute Int/Buf result -> format/write
```

The final expression should normally be `0` after successful output. If the
task asks for an exit code, return or call `@exit` according to the examples.

### Maintaining Round-Trip Stability

Round-trip stability means the authoring view can parse to the AST and render
back with the same binding names and layout intent. This matters for repair
tasks because a structural edit should not rename every local or reorder
unrelated code. Keep these habits:

Use stable helper names. A recursive helper named `loop` is fine when there
is one loop. If there are two, use names such as `outer`, `inner`, `scan`, or
`emit`.

Keep buffer names stable. `buf` for input, `out` for formatted output, `tmp`
for scratch, and `dst`/`src` for copy operations reduce argument-order
mistakes.

Keep branch bodies local. A long `if` branch should usually bind a value with
`let` before returning it, instead of embedding several nested calls directly
inside the branch expression.

Keep recursive state explicit. A repair model can change `acc` or `i` more
reliably than it can decode a packed arithmetic state. Pack state only when
the program is otherwise too awkward under the current surface.

### Failure Triage Examples

If the checker reports `type-mismatch` at a `@buf-get`, inspect the first
argument. It must be a `Buf`, not an integer length or file descriptor. For
`@read`, the first argument is the file descriptor and the second is the
buffer; for `@buf-get`, the first argument is the buffer and the second is
the offset.

If the checker reports `operator-overload-failure`, inspect both operands
after any comparison call. A common mistake is to feed a boolean from `@eq`
or `@lt` into `@add`. Use an `if` to turn a boolean into `1` or `0`.

If the checker reports `unresolved-type` for a lowercase name, the parser
probably turned an unbound variable into a hole or the typechecker saw an
unknown primitive. Bind the name with `let`, move it into lambda scope, or use
one of the allowed primitive names.

If codegen reports an unsupported node, simplify toward the Stage 3/4 codegen
subset. Records and projections can typecheck but are not the best shape for
Phase 3 executable references. Prefer integer state, buffers, `let`, `if`,
`match`, `lambda`, `rec`, and primitive calls.

If tests fail but compile and typecheck pass, reason from input bytes. Most
wrong Phase 3 outputs are one of: off-by-one length, forgetting to flush the
last number at EOF, writing the whole output buffer instead of `w` bytes,
mixing destination and source in `@buf-copy`, or using ASCII byte values
without subtracting `48` for digits.

### Metrics-Aware Writing

The Phase 3 token metric counts the primer plus generated Tacit for every
task. That does not mean the generated program should be cryptic. A program
that passes is worth more than a short program that fails. Use the canonical
idioms from this primer first. Only shorten after the algorithm is obviously
correct.

Avoid token tricks that hurt repair: meaningless one-letter names everywhere,
packed state with no clear invariant, unnecessary aliases for primitives, and
deeply nested expressions where one `let` would name the intermediate value.
The reference style balances compactness and editability because the same
primer is used for generation and for maintenance/repair evaluation.

### Boundary Conditions To Remember

Empty input: `@read` returns zero immediately. Make the EOF branch return the
right identity value: count zero, sum zero, longest length zero, or the
current accumulated value if the last token has no trailing newline.

Single element: sorting, unique, longest, and prefix-shaped tasks often fail
on one-element input when the recursive step assumes a successor exists.

Trailing newline: line-oriented programs should decide whether a final empty
line counts. Follow the task statement, not a generic Python habit.

ASCII digits: byte `48` is digit zero and byte `57` is digit nine. Convert a
digit byte with `@sub byte 48`. Convert back for output either with
`@fmt-i64` for whole integers or by adding `48` for a single digit byte.

Negative numbers: `@parse-i64` handles the primitive contract for integer
text. If hand-scanning bytes, account for a leading minus sign explicitly.

Buffer lengths: write and compare exactly the logical length, not the
allocated capacity. Allocating 32 bytes for formatting does not mean all 32
bytes are valid output.

### Model Output Format

When answering an evaluation task, output only the Tacit-Lite program unless
the harness explicitly asks for explanation. The extractor looks for Tacit
source, not a tutorial. Do not include sidecar TOML in the generated answer;
the evaluation path measures the authoring-view program and applies the
checker/compiler to that text. Do not wrap the program in Markdown unless the
caller asks for a fenced block.

The safest generated program shape is:

```text
let helper = ... in
let input_or_buffer = ... in
let result = ... in
let out = @buf-alloc 32 in
let w = @fmt-i64 out 0 result in
let _ = @write 1 out w in
let _ = @write 1 "\n" 1 in
0
```

Use this skeleton for integer-output tasks. For tasks that output a string or
transformed bytes, compute the output buffer and logical length, then write
that buffer and length directly. Do not format a buffer as an integer unless
the required output is numeric.

### Working Without Tests In Context

The model does not receive hidden tests. Use the task statement to infer edge
cases. If the statement says lines, handle empty input and trailing newline.
If it says integers, handle zero, one value, and negative signs when the
reference language would. If it says sorted output, preserve duplicates
unless the statement says unique. If it says first occurrence, do not update
the answer after finding an equal later occurrence.

Because the compiler can only check syntax, types, effects, and codegen
support, most semantic bugs survive until test execution. Before finalizing a
program, mentally run it on: empty input, one token, two tokens, already
sorted input, reverse sorted input, duplicated values, and a final token
without newline. These cases cover most Phase 3 task failures.

### Choosing A Recursion State

A good recursive state has a small invariant that can be spoken in one
sentence. Examples: `i` is the next offset to inspect; `acc` is the sum of
all complete numbers read so far; `best` is the longest length seen so far;
`out_len` is the number of valid bytes already written to `out`.

Avoid states that require reconstructing history. If a helper needs to know
whether it is in a word, carry an `in_word` flag. If it needs the start of
the current line, carry the start offset. If it needs to output separators,
carry a `first` flag so the branch can decide whether to write the separator.

When a state has more than two values, nested lambdas are often clearer than
packing. A helper can be called as `loop i acc best flag`. The compiler
lowers closed multi-argument helpers directly in Phase 3; you do not need to
avoid this style for performance.

### Byte-Oriented String Work

Tacit-Lite strings at the primitive boundary are byte sequences. For
algorithmic string work, copy bytes into buffers and operate on integer byte
values. ASCII lowercase letters are `97` through `122`; uppercase letters are
`65` through `90`; space is `32`; newline is `10`.

To lowercase an ASCII uppercase byte, check `@ge byte 65` and `@le byte 90`,
then add `32`. To detect a digit, check `@ge byte 48` and `@le byte 57`.
To detect whitespace in small tasks, compare against the explicit bytes the
task permits, commonly space and newline. There is no Unicode normalization
surface in Phase 3.

When constructing output, always maintain an output offset. A common pattern
is: write a byte with `@buf-set out off byte`, then recurse with `@add off
1`. If writing a multi-byte formatted integer, use the length returned by
`@fmt-i64` to advance the offset or write immediately.

### Algorithm Selection Rules

Prefer the simplest algorithm that fits the primitive surface. For small
input sizes and byte buffers, quadratic algorithms are often acceptable and
clear. A clever asymptotic improvement that needs a hash map, heap, or
general list library is not a Phase 3 win because those abstractions are not
available.

For search tasks, linear scan is the default unless the task explicitly gives
sorted input and asks for binary search. For grouping tasks, sorted or
stable-order buffer passes are often clearer than inventing a dictionary.
For matrix-like tasks, compute row and column offsets manually and keep the
dimension variables named. For dynamic programming tasks, be wary: if the
state table would be large, the task may be intentionally stdlib-dominated
and the explicit Tacit version will be longer.

### Debugging Generated Tacit

If a program fails to parse, reduce the nearest expression. Parentheses are
needed around compound arguments, especially recursive calls and nested
`if`s. A `let` right-hand side extends until `in`, so missing `in` often
makes the parser recover far away from the actual mistake.

If a program typechecks but codegen fails, remove surface features that are
type-level only or not in the codegen subset. Favor `Int` results, buffers,
primitive calls, and direct helper calls. Avoid returning records, functions,
or constructors from the final expression in executable examples.

If a program passes simple cases and fails larger cases, inspect buffer
capacity and output length. `@buf-alloc 32` is enough for one formatted i64,
not for an arbitrary output string. For output that grows with input, allocate
based on an input-size estimate or reuse the input buffer when the
transformation is in-place and safe.

### Metrics Schema Reminder

Phase 3 result files embed diagnostics under each task result. A compile or
typecheck failure stores the same `p2.0` envelope shown above. A runtime test
failure stores a synthetic diagnostic with kind `test-failure`. The metrics
file also records token counts: fixed primer tokens at `model.primer_tokens`,
per-task generation tokens, and per-task Python baseline tokens. The gate
uses primer-inclusive per-task Tacit cost: every task pays the full primer
again because every model invocation receives the full primer.

### What Not To Infer

Do not infer features that are intentionally absent. No list syntax, no
implicit string iteration, no Python-style slicing, no automatic stdout
printing, no mutable locals except through explicit buffers, no heap
allocation, and no hidden standard library. If the task needs a table, encode
the needed state directly with buffers or integers. If the task needs a
string operation, use byte buffers and the Phase 3 primitives. If a shorter
solution would need a missing abstraction, write the explicit one and let the
metrics report the cost.
