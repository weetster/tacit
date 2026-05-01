# Tacit-Lite Primer

## Output Contract

When asked for a fenced Tacit-Lite program, return exactly one fenced block
tagged `tacit`, with only Tacit-Lite source inside it. Do not write a plan,
explanation, checklist, second candidate, corrected version, tests, comments,
or any other Markdown. Multiple `tacit` blocks and unfinished responses are
hard failures. If uncertain, choose the simplest implementation that fits the
primitive surface and finish the one block. If an implementation is growing
too long, switch to a smaller complete algorithm before writing the block; a
finished simple program is better than a partial elaborate program.
Never show scratch work before the block. Never open a second block to revise
the first one. The final answer starts with ```` ```tacit ```` and ends with
the matching closing fence.

## 1. Semantic Summary

Tacit-Lite is a small expression language. Authors write source using `let`,
`lambda`, `rec`, `if`, `match`, records, and `@name` primitive calls. The
compiler parses that source, typechecks it, emits native code, and can render
it back while preserving author-facing names.

A Tacit program is usually one expression. A binding extends only the body
after `in`. A lambda has exactly one parameter, so multi-argument functions
are curried: `lambda x. lambda y. ...`, then called as `f a b`. Recursive
helpers use `rec {name = lambda ...; ...} in body`. A `rec` group is the
only way a function can call itself or a sibling helper. A helper must be
called with all of its source-level arguments at each executable call site:
`loop next_i next_acc`, not `loop next_i`, when the helper was defined as
`lambda i. lambda acc. ...`.

Type inference is local. Standalone examples in this primer rely on
inference. The base runtime values are `Int`, `Bool`, `Str`, `Buf`, records,
constructors, lambdas, and holes. The effect lattice has four atoms: `Alloc`,
`Mut`, `IO`, and `Div`. Pure code has `{}`.
Allocation of stack buffers adds `Alloc`; buffer writes and integer
formatting add `Mut`; `@read`, `@write`, and `@exit` add `IO`; recursive
calls and division-like primitives can add `Div`.

There is no implicit mutable state. Mutation is explicit through `Buf`
primitives. A `Buf` is a byte buffer: each `@buf-get` reads one byte-sized
integer. Keep counters, offsets, indexes, and other large `Int` values in
lambda parameters or `let` bindings, not in buffer cells. There is no general
heap buffer, hash map, object system, type class, effect handler, or
user-defined effect in Tacit-Lite. If a task wants those, write the direct
Tacit-Lite shape with the available primitives.

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

`Buf` is Tacit-Lite's mutable byte buffer. Bind writes to `_` when only the
mutation matters.

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

1. Identify the value that the whole program returns. In typical executable
   Tacit-Lite programs this is an `Int`, often after printing bytes to stdout.
2. Pull named helper functions out before writing the main expression. A
   helper that calls itself or another helper in its group goes in `rec`; a
   helper that does not recurse is a `let`.
3. Replace loops with recursive helpers or with a fixed sequence of buffer
   operations when the size is static. Tacit-Lite has no `while` or `for`
   keyword.
4. Replace Python lists, bytearrays, and Rust arrays with `Buf` only when the
   stored values are bytes or small flags. Use explicit offsets. Keep full
   `Int` values in recursive state or local bindings.
5. Replace standard-library parsing and formatting with `@parse-i64` and
   `@fmt-i64`. Do not hand-roll those unless the task is specifically about
   parsing or formatting internals.
6. Check the effect story last. If the program reads or writes, it has `IO`.
   If it allocates a buffer, it has `Alloc`. If it writes to a buffer, it has
   `Mut`. If it recurses or divides, it may have `Div`.

For structured stdin, prefer one `@read` into a buffer and parse by offsets.
`@read` is a byte read, not a line reader or token reader. Calling `@read`
twice usually consumes whatever remains after the first call; it does not mean
"read line 1, then read line 2". For two input lines, read once and find the
newline:

```tacit
let buf = @buf-alloc 64 in
let n = @read 0 buf 64 in
let nl = @scan-byte buf 0 n 10 in
let a = @parse-i64 buf 0 nl in
let b = @parse-i64 buf (@add nl 1) (@sub n (@add nl 1)) in
@add a b
```

`@scan-byte buf off len byte` returns an absolute offset. If the byte is not
found, the result is `off + len`. Do not add `off` to the result again. When
scanning inside a line, pass the remaining length as `@sub line_end off`, not
the absolute end offset.

Tacit code is densest when it keeps the computation in expression form. A
long chain of `let`s is normal and readable. Avoid translating statement for
statement when a helper can express the loop state directly. For example, a
Python loop with `total`, `i`, and `n` usually becomes `rec {loop = lambda
state. ...}` where `state` is an encoded integer or one parameter per value,
depending on which shape is clearer.

### Reading Tacit Application

Application has no comma syntax. Read `f a b c` as `(((f a) b) c)`.
Parentheses are only needed when an argument is itself a compound expression,
as in `@mul n (fact (@sub n 1))`. If a call is rejected because an `Int` was
expected but a function was found, first check for a missing final argument.
This is common in recursive helpers with two or more state values.

Primitive names should remain primitive names. A wrapper like `let plus =
lambda x. lambda y. @add x y in ...` is useful only when it participates in a
larger abstraction. It is not useful as a synonym for `@add`.

### Branch Syntax Traps

Every `if` is an expression and must have both `then` and `else`. There is no
brace block syntax. The expression immediately after `then` must be an atom or
application; if the then-branch begins with `let`, `if`, `rec`, `match`, or
`lambda`, wrap that whole branch in parentheses. The `else` branch may be a
full expression, but parenthesizing compound branches on both sides is often
clearer. Do not rely on indentation or line breaks to group a branch. Every
inner `if` must have its own `else` before the surrounding `let`, `rec`, or
match arm closes.

Correct compound then-branch:

```tacit
let out = @buf-alloc 2 in
let _ = if @eq 1 1 then
  (let _ = @buf-set out 0 65 in
   @buf-set out 1 10)
else 0 in
@buf-get out 0
```

Wrong shape: `if cond then let x = ... in ... else ...`. The parser reads the
`then` branch too narrowly and later reports `expected 'else'`.

Safer branch rule: when either side is compound, parenthesize both sides.

```text
if cond then
  (let x = value in result)
else
  (if other then a else b)
```

### Choosing Between `if` And `match`

Use `if` when there is one condition and two outcomes. Use `match` when the
branches correspond to values or constructors. In small programming tasks,
integer matching is useful for sentinels, parser states, and compact
zero/non-zero cases where the branch names are clearer than nested
comparisons.

### Buffer Rules

Treat `Buf` as a capability-like handle scoped by `let`. A buffer is created
by `@buf-alloc` or `@buf-alloc-dyn`, then passed explicitly to every read or
write primitive. There is no implicit current buffer and no indexing sugar.
The index is always an integer argument. A buffer cell stores a byte value,
not a full arbitrary `Int`; use buffers for input bytes, output bytes, and
small flags, and use recursive parameters for large numeric state. Because
the buffer handle is not a general heap object, keep it inside the expression
that owns it; do not try to return a closure that stores a buffer for later
use.

Do not build an integer array with `@buf-set vals i value` unless `value` is
known to stay in `0..255`. Negative numbers and values above 255 will not be
stored as full integers. For sorting or indexed access to parsed integers,
prefer one of these shapes: rescan the input to find the nth token, store byte
offsets into the original input using multiple buffer cells per offset, or
carry the few needed integer values in recursive state.

For recursive scans over a buffer, allocate the buffer outside the `rec` group
and refer to that buffer by name inside the helper. Use source-level helper
parameters for changing integer state: offsets, lengths, counters, flags, and
accumulators. Do not make the buffer itself a lambda parameter in a recursive
helper.

```text
let buf = @buf-alloc-dyn n in
rec {scan = lambda i. lambda acc. ... @buf-get buf i ... scan next_i next_acc} in scan 0 0
```

If you need to output a slice that starts at a nonzero offset, do not overwrite
the input buffer while later scans still need it. Either copy the slice into a
separate output buffer with `@buf-copy out 0 input start len` and then write
`out len`, or emit one byte at a time through a one-byte scratch buffer.

### Output Rules

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

Put sibling recursive helpers in one `rec` group instead of creating a fresh
`rec` inside a recursive helper body. A nested helper that needs `i`, `end`,
or another changing value should usually become a sibling and receive that
value as a parameter. This keeps parsing simple and keeps every changing part
visible at the call site.

Use `if` for two branches selected by a comparison or truthy integer.

```tacit
let n = 9 in if @gt n 3 then @sub n 3 else 0
```

Use `match` when the shape is a pattern. Integer patterns and `_` are the
most common executable uses.

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

When a recursive helper needs an outer value that was bound before the `rec`
group, refer to that value by name. Keep explicit parameters for state that
changes on each call. Do not hide changing loop state inside a nested helper;
pass it as `loop i acc best flag` or as a sibling helper argument.

```tacit
let base = 40 in
rec {plus_base = lambda x. @add x base} in plus_base 2
```

### Names

Names matter for readability. Prefer names that describe the local role: `n`,
`i`, `len`, `buf`, `out`, `w`, `loop`, `state`, `acc`, `head`, `tail`,
`left`, `right`. Do not rename only to shave tokens. Write names that are
easy to reason about during repair.
Local identifiers use letters, digits, and underscores. Do not put hyphens in
local names: write `find_newline`, not `find-newline`. Hyphens are valid in
primitive names after `@`, such as `@parse-i64`, and in negative integer
literals such as `-1`.

### Local State Encoding

When a helper needs several small integer state values and Tacit syntax makes
multi-parameter recursion awkward for a tiny example, an encoded integer
state can be acceptable. For example, one integer can pack a high part and a
low part so a loop can carry both a running sum and a current number. Use
that sparingly: it is compact, but it is harder to repair. If two nested
lambdas are clearer, prefer them:

```text
rec {loop = lambda i. lambda acc. if done then acc else loop next_i next_acc} in loop 0 0
```

This is `text` because `done`, `next_i`, and `next_acc` are placeholders.

### Records And Projection

Records are useful for pure grouping, but executable programs are
not designed around a large record-heavy style. Use records when a function
naturally returns a small bundle that is consumed immediately. Field order is
not semantic. If you project a field, make sure the inferred record type
actually contains that field.

### Constructors And Patterns

Capitalized identifiers are constructors. `True` and `False` are known
nullary boolean constructors. Other constructors can appear in parsed syntax
and patterns, but executable programs should usually avoid algebraic data
construction at runtime because the executable subset is intentionally small.
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

### Program Boundary

The program is authoring-view Tacit source. If a surrounding declaration says
the program returns `Int` with effects `{Alloc, IO, Mut}`, then the source
expression must infer `Int` with those effects. If the program prints but the
declaration omits `IO`, the checker reports an effect violation.

## 4. Effect Reasoning

A pure program has no allocation, mutation, IO, or possible divergence beyond
ordinary finite execution.

```tacit
let square = lambda x. @mul x x in square 6
```

This example is pure because `@mul` is pure and calling `square` does not use
any effectful primitive. A boundary declaration for this expression would use
the empty effect set.

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

The full set is `{Alloc, IO, Mut}`. Any declaration for this program must
include all three atoms.

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

A recursive helper carries `Div` through its call effect. Tacit-Lite does not
prove termination.

```tacit
rec {count = lambda n. if n then count (@sub n 1) else 0} in count 3
```

Effect errors usually mean the declaration or annotation under-declared the
effect set. The fix is to include the inferred atoms, not to hide the
primitive call.

### Effect Join Rules

Effects join by union. If one part of a program is `{Alloc}` and a later part
is `{Mut}`, the whole expression has `{Alloc, Mut}`. When effects are written
in declarations, use the stable alphabetic order: `Alloc`, `Div`, `IO`, `Mut`.

Partial application matters. `@write 1` is just a pure function value waiting
for the buffer and length. The `IO` effect appears only at the fully applied
call. This is why helper construction can remain pure even if calling the
helper later performs IO.

`let` joins the effect of its right-hand side with the effect of its body.
`if` joins the condition, then branch, and else branch. `match` joins the
scrutinee and every arm body. A lambda expression itself is pure to create;
the effect is attached to the function call. A recursive function's call
effect includes `Div` because Tacit-Lite does not prove recursion terminates.

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
set, do not delete useful work to satisfy a too-small declaration. The
declared effects are the boundary claim. The source decides the truth. Add
missing effects to the declaration or annotation if the program is otherwise
correct.

When an effect appears surprising, find the innermost primitive that creates
it. `Mut` usually comes from `@buf-set`, `@buf-copy`, `@fmt-i64`, or
`@read`. `IO` comes from `@read`, `@write`, or `@exit`. `Alloc` comes from a
buffer allocation primitive. `Div` comes from recursion, division, or modulo.

## 5. Negative Examples And Diagnostics

Each failing Tacit block below is marked with the diagnostic kind it should
produce.

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

Diagnostic kind: `unresolved-type`. Fix: use a primitive from Tacit-Lite's
primitive surface, or bind a lowercase helper name before using it.

```tacit
@add 1 0
```

### Diagnostic Reading Pattern

The important repair signals are `kind`, `message`, `expected`, and `actual`.
For repair, read the first error, fix the smallest local expression that can
cause it, then rerun the checker. Later errors may disappear after the first
hole or type mismatch is fixed.

### Negative Example Reading Order

For `type-mismatch`, inspect the nearest argument, annotation, branch, field,
or primitive call. The checker reports the type it expected and the type it
inferred. If either side is `Unknown`, a previous diagnostic probably hid the
real source.

For `operator-overload-failure`, keep the operator and fix the operands.
Arithmetic operators want integer operands.
Comparison operators also compare integers and return booleans.

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
integers.

`unresolved-type`: a type name, constructor, field projection, or unknown
primitive-like symbol is not known to the typechecker. Use a known base type,
field, constructor, or primitive.

`unbound-type-variable`: a `ty-var` appears without an enclosing `forall`.
In authoring-view examples, avoid raw type variables unless the surrounding
type expression binds them.

`type-arity-mismatch`: a type constructor was applied to the wrong number of
arguments. Tacit-Lite programs are usually monomorphic;
avoid unnecessary type application.

`module-missing-annotation`: an exported module binding lacks a type/effect
signature. This is a warning. Add the missing signature where modules are
used.

`effect-violation`: an inferred effect set is not a subset of the declared
effect set. Add the missing atoms, commonly `Alloc`, `Mut`, `IO`, or `Div`.

`unbound-effect-variable`: an `eff-var` appears without an enclosing
effect-polymorphic `forall`. Tacit-Lite programs should usually use concrete
effect sets instead.

`buf-escape`: a buffer handle is used outside the scope where the checker can
prove it is valid. Keep buffer use inside the `let` body that owns it.

`parse-error`, `unexpected-token`, `expected-expr`, `unclosed-paren`,
`expected-pattern`, `unbound-name`, and `arity-mismatch` come from parser
recovery holes. Fix the local syntax first; later diagnostics may be
consequences of the hole.

### Repair Checklist

When a program fails, use this sequence:

1. Parse error or hole diagnostic: fix the local authoring syntax.
2. Unknown name or primitive: bind the lowercase name, capitalize a real
   constructor, or use a primitive from Tacit-Lite's surface.
3. Type mismatch at a primitive: check argument order. Buffer primitives are
   strict about `buf`, `offset`, `length`, and `value` positions.
4. Branch mismatch: make both branches return the same type.
5. Effect violation: update the boundary effect set to match source behavior.
6. Unsupported executable shape: rewrite toward the executable subset:
   integer result, direct helper calls, `rec`, `if`, `match`, `let`, and
   supported primitives.
7. Test failure after compile success: keep the type/effect shape and debug
   the algorithm with smaller input.

### Common Task Recipes

These recipes are language-level patterns. They name the shapes that appear
across small programming tasks without assuming any external file layout.

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

For separated output, choose one separator rule and use it consistently:
write a separator before every item except the first, or after every item
except the last. Do not do both. A `first` flag or column index is often the
cleanest way to avoid doubled spaces.

Searching a buffer: prefer `@scan-byte` for a byte target and `@buf-eq` for a
slice equality check. Hand-written recursive scans are acceptable when the
predicate is richer than equality, but they are longer and more error-prone.
Keep offsets and lengths explicit. `@scan-byte` returns the absolute found
offset, so the next byte after a found newline is `@add end 1`, not `@add
start (@add end 1)`.

When scanning bytes read from stdin, treat the byte count returned by `@read`
as the exclusive upper bound. A helper with state `i` should inspect
`@buf-get buf i` only while `i < n`; when `i == n`, return the EOF result.

Copying a slice: use `@buf-copy dst dst_off src src_off len`. Remember that
the first buffer is the destination. Many wrong answers reverse `dst` and
`src`; the typechecker cannot catch that because both are `Buf`.

Sorting a fixed tiny buffer: direct buffer reads and writes can be clearer
than a general recursive sort. For a dynamic or task-shaped size, write a
recursive outer loop and inner loop, or choose a simple algorithm whose state
fits cleanly in integers and buffers. Prefer correctness over clever token
packing.

For large-output transforms such as sorting lines or grouping items, prefer a
simple complete quadratic pass over a clever partial divide-and-conquer shape.
Direct recursion with explicit indexes is easier to finish and repair than a
program that needs lists, heaps, or a dictionary.

Token streams of signed integers: define `skip`, `token_end`, and `parse_at`
helpers over the original input. `skip pos` advances past spaces and
newlines. `token_end pos` stops at space, newline, or EOF. `parse_at p` calls
`@parse-i64 input p (@sub (token_end p) p)`. This handles negative signs
without hand-scanning digits and avoids corrupting full integers in byte
buffers.

Longest-word and common-prefix tasks: keep byte offsets and lengths into the
input, not a packed integer encoding of the word. For longest word, carry
`cur_start`, `cur_len`, `best_start`, and `best_len`; on a tie, keep the old
best if the task asks for the first occurrence. At the end, copy `best_len`
bytes from `best_start` into an output buffer or emit those bytes.

Binary search over a token line: count tokens first, then search with a
half-open interval `[lo, hi)`. Stop when `lo >= hi`. Compute `mid = (lo + hi)
/ 2`, parse the value at token `mid`, then recurse into `[mid + 1, hi)` or
`[lo, mid)`. A closed interval is easy to get wrong at the empty range.

Maximum-subarray over parsed integers: parse the first integer before the
main loop and initialize both `cur` and `best` to that value. Then for each
remaining value `x`, set `cur = max(x, cur + x)` and `best = max(best, cur)`.
Do not initialize `best` to zero because all-negative input must return the
largest negative value.

Comparing flags: comparison primitives return `Bool`, while integer
truthiness is also accepted in conditions. Arithmetic expects integers. If
you need to add a flag, convert through an `if`: return `1` for true and `0`
for false.

### Input/Output Discipline

Tacit-Lite has no implicit command-line argument parser. Portable programs
receive input on stdin and write bytes to stdout. A program should
not assume environment variables, files by path, or a process argument vector
unless the caller explicitly provides such a primitive. The portable pattern
is:

```text
read stdin bytes -> parse or scan -> compute Int/Buf result -> format/write
```

The final expression should normally be `0` after successful output. If the
task asks for an exit code, return that code or call `@exit`.

For multi-value input, read once when practical and parse slices. Use
`@scan-byte buf off len 10` to find a newline and `@scan-byte buf off len 32`
to find a space. The result is the found absolute offset, or `off + len` when
the byte is absent. Parse `@parse-i64 buf start (@sub end start)`. This avoids
the common bug where the first large `@read` consumes both lines and a second
`@read` sees EOF.

### Keeping Source Easy To Repair

Generated source is easier to repair when names, helper shapes, and branch
structure stay stable. Keep these habits:

Use stable helper names. A recursive helper named `loop` is fine when there
is one loop. If there are two, use names such as `outer`, `inner`, `scan`, or
`emit`.

Keep buffer names stable. `buf` for input, `out` for formatted output, `tmp`
for scratch, and `dst`/`src` for copy operations reduce argument-order
mistakes.

Keep branch bodies local. A long `if` branch should usually bind a value with
`let` before returning it, instead of embedding several nested calls directly
inside the branch expression.

Keep recursive state explicit. A repair pass can change `acc` or `i` more
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

If a program uses an unsupported executable shape, simplify toward the
executable subset. Records and projections can typecheck but are not the best
shape for portable executable programs. Prefer integer state, buffers, `let`,
`if`, `match`, `lambda`, `rec`, and primitive calls.

If tests fail but compile and typecheck pass, reason from input bytes. Most
wrong outputs are one of: off-by-one length,
forgetting to flush the last number at EOF, writing the whole output buffer
instead of `w` bytes, mixing destination and source in `@buf-copy`, or using
ASCII byte values without subtracting `48` for digits.

For base conversion, the first digit computed by repeated division is usually
the least significant digit. If the output expects most-significant first,
either fill an output buffer from right to left or do a second pass that emits
the temporary digits in reverse order.

### Token-Aware Writing

Compactness matters because Tacit is meant to be economical for models to
read and write. That does not mean the generated program should be cryptic. A
program that passes is worth more than a short program that fails. Use the
recommended idioms from this primer first. Only shorten after the algorithm is
obviously correct.

Avoid token tricks that hurt repair: meaningless one-letter names everywhere,
packed state with no clear invariant, unnecessary aliases for primitives, and
deeply nested expressions where one `let` would name the intermediate value.
The recommended style balances compactness and editability because generated
programs often need later repair.

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

UTF-8 text: byte loops are correct for ASCII tasks. If a task is explicitly
about characters and may include non-ASCII input, do not reverse or split the
middle bytes of a multi-byte sequence. Copy each UTF-8 sequence as a unit. For
character palindrome checks, the last byte is not necessarily the last
character. Move the right pointer left over continuation bytes in `128..191`,
compare the whole byte span for the left and right characters, then advance by
the span lengths.

Negative numbers: `@parse-i64` handles the primitive contract for integer
text. If hand-scanning bytes, account for a leading minus sign explicitly.

Buffer lengths: write and compare exactly the logical length, not the
allocated capacity. Allocating 32 bytes for formatting does not mean all 32
bytes are valid output.

### Output Format

When asked to produce a Tacit-Lite program, output only the requested program.
If the caller asks for a fenced block, return exactly one `tacit` fenced block
and nothing else. Do not include reasoning, prose, auxiliary declarations,
tests, or an alternate implementation before or after the block. Do not open a
second `tacit` block. Do not leave the block unfinished.

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

### Checking Edge Cases

Use the problem statement to infer edge cases. If the statement says lines,
handle empty input and trailing newline. If it says integers, handle zero, one
value, and negative signs when the equivalent Python or Rust solution would.
If it says sorted output, preserve duplicates unless the statement says
unique. If it says first occurrence, do not update the answer after finding an
equal later occurrence.

Because static checks cover syntax, types, effects, and executable-shape
support, most semantic bugs survive until execution. Before finalizing a
program, mentally run it on: empty input, one token, two tokens, already
sorted input, reverse sorted input, duplicated values, and a final token
without newline.

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
lowers closed multi-argument helpers directly; you do not need to avoid this
style for performance.

### Byte-Oriented String Work

Tacit-Lite strings at the primitive boundary are byte sequences. For
algorithmic string work, copy bytes into buffers and operate on integer byte
values. ASCII lowercase letters are `97` through `122`; uppercase letters are
`65` through `90`; space is `32`; newline is `10`.

To lowercase an ASCII uppercase byte, check `@ge byte 65` and `@le byte 90`,
then add `32`. To detect a digit, check `@ge byte 48` and `@le byte 57`.
To detect whitespace in small tasks, compare against the explicit bytes the
task permits, commonly space and newline. Tacit-Lite has no Unicode
normalization surface.

When constructing output, always maintain an output offset. A common pattern
is: write a byte with `@buf-set out off byte`, then recurse with `@add off
1`. If writing a multi-byte formatted integer, use the length returned by
`@fmt-i64` to advance the offset or write immediately.

### Algorithm Selection Rules

Prefer the simplest algorithm that fits the primitive surface. For small
input sizes and byte buffers, quadratic algorithms are often acceptable and
clear. A clever asymptotic improvement that needs a hash map, heap, or
general list library is not a win because those abstractions are not
available.

For search tasks, linear scan is the default unless the task explicitly gives
sorted input and asks for binary search. For grouping tasks, sorted or
stable-order buffer passes are often clearer than inventing a dictionary.
For matrix-like tasks, compute row and column offsets manually and keep the
dimension variables named. For dynamic programming tasks, be wary: if the
state table would be large, the task may be dominated by missing standard
library support and the explicit Tacit version will be longer.

For nested row/column loops, use the bounds literally. If rows are `0..r` and
columns are `0..c`, the outer loop stops at `r` and the inner loop stops at
`c`. Single-row, single-column, and single-cell cases should still run the
body once.

### Debugging Generated Tacit

If a program fails to parse, reduce the nearest expression. Parentheses are
needed around compound arguments, especially recursive calls and nested
`if`s. A `let` right-hand side extends until `in`, so missing `in` often
makes the parser recover far away from the actual mistake.

If a program typechecks but cannot run as an executable, remove surface
features that are type-level only or not in the executable subset. Favor
`Int` results, buffers, primitive calls, and direct helper calls. Avoid
returning records, functions, or constructors from the final expression in
executable programs.

If a program passes simple cases and fails larger cases, inspect buffer
capacity and output length. `@buf-alloc 32` is enough for one formatted i64,
not for an arbitrary output string. For output that grows with input, allocate
based on an input-size estimate or reuse the input buffer when the
transformation is in-place and safe.

### What Not To Infer

Do not infer features that are intentionally absent. No list syntax, no
implicit string iteration, no Python-style slicing, no automatic stdout
printing, no mutable locals except through explicit buffers, no heap
allocation, and no hidden standard library. If the task needs a table, encode
the needed state directly with buffers or integers. If the task needs a
string operation, use byte buffers and the primitives listed in this primer.
If a shorter solution would need a missing abstraction, write the explicit
one.
