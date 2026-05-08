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
the first one. Do not discuss feasibility, constraints, memory estimates, or
alternative approaches outside the block. If you need to revise while
thinking, do it silently and return only the final program. The final answer
starts with ```` ```tacit ```` and ends with the matching closing fence.

Length pressure is not permission to split the answer. For a large ordering,
row/column, line-processing, or grid traversal program, choose one complete
direct shape and finish it. Do not provide a short version plus a more
complete version. Do not leave a `let`, `rec`, `if`, or fenced block open. If
the program is getting too long, simplify the algorithm before writing the
final block: rescan input instead of storing a complex table, use one clear
quadratic pass instead of a half-written divide-and-conquer routine, and
prefer direct output over building a large abstract result.

## Hard Rules (Critical for Correctness)

1. **Final expression must be `0`.** Executable programs return an exit status.
   The final expression must evaluate to the literal integer `0`. When using
   `@write`, note that it returns the byte count (e.g., 1 for writing `"\n"`).
   If the final expression is `@write 1 "\n" 1`, the process exits with status
   `1`, causing a test failure. Always end with an explicit `0` unless the task
   explicitly requests a non-zero exit code.

2. **Never put `@` in front of a name you define.** The `@`-prefix is reserved
   for the primitive surface listed in §3 (Primitive Surface). If you write
   `@search_from` or `@buffer_copy`, the compiler will report an
   unknown-primitive error. Defined helpers, parameters, and local bindings use
   bare names without `@`.

3. **`rec` groups are mutually visible by name only, not by parameter scope.**
   If a `rec` group has `bubble = lambda pass. ... inner ...` and
   `inner = lambda i. ... pass ...`, the name `pass` inside `inner` is unbound.
   The parameter `pass` of `bubble` is not visible in `inner`'s scope. Either
   pass `pass` as an explicit parameter to `inner` (and to all call sites of
   `inner`), or inline the inner loop into `bubble`'s body.

4. **Cap allocations at ~1 MB.** Calls like `@buf-alloc 16777216` (16 MB) or
   `@i64-alloc 2097152` (also 16 MB; each cell is 8 bytes) crash before the
   program runs. Unless the task statement explicitly requires more, cap
   `@buf-alloc` counts at `1048576` and `@i64-alloc` counts at `131072`. A full
   Unicode codepoint table (`@i64-alloc 1114112` or larger) is over the cap;
   sort decoded codepoints with `@sort-i64` instead, or compute the count from
   the input length. Static allocations (tuples, records) do not have this
   constraint.

5. **String literal escapes are limited to `\n`, `\t`, `\r`, `\\`, `\"`, and
   `\u{HEX}`.** Inside `"..."`, the only recognized escapes are those six.
   `\x..`, `\f`, `\v`, `\0`, and `\'` raise `lex: bad escape`. To embed
   another byte, write it as `\u{HEX}` with lowercase hex digits, or build
   the byte sequence with `@buf-set`. Most input-classification work needs
   only `" \t\n\r"`; form feed and vertical tab rarely appear and can be
   handled by direct byte comparison (`@eq b 12`, `@eq b 11`) when needed.

## 1. Semantic Summary

Tacit-Lite is a small expression language. Authors write source using `let`,
`lambda`, `rec`, `if`, `match`, records, first-class function values, and
`@name` primitive calls. The compiler parses that source, typechecks it,
emits native code, and can render it back while preserving author-facing
names.

A Tacit program is usually one expression. A binding extends only the body
after `in`. A lambda has exactly one parameter, so multi-argument functions
are curried: `lambda x. lambda y. ...`, then called as `f a b`. Recursive
helpers use `rec {name = lambda ...; ...} in body`. A `rec` group is the
only way a function can call itself or a sibling helper. A helper must be
called with all of its source-level arguments at each executable call site:
`loop next_i next_acc`, not `loop next_i`, when the helper was defined as
`lambda i. lambda acc. ...`.

Every `rec` member must be a lambda. Do not put constants, buffers, parsed
values, or other computed expressions inside a `rec` group as members. Bind
those values before or after the group with `let`. Inside the group, each
helper should either return an `Int`/`Bool`-shaped value or perform direct
buffer/IO work and return an `Int` status.

Type inference is local. Standalone examples in this primer rely on
inference. The base runtime values are `Int`, `Bool`, `Str`, `Buf`, `I64Vec`,
records, constructors, and lambdas. Lambdas are function values: they can be
passed, returned, stored in records, and called through variables. The effect
lattice has four atoms: `Alloc`, `Mut`, `IO`, and `Div`. Pure code has `{}`.
Allocation of stack buffers adds `Alloc`; buffer writes and integer
formatting add `Mut`; `@read`, `@write`, and `@exit` add `IO`; recursive
calls and division-like primitives can add `Div`.

There is no implicit mutable state. Mutation is explicit through `Buf` and
`I64Vec` primitives. A `Buf` is a byte buffer: each `@buf-get` reads one
byte-sized integer. Keep counters, offsets, indexes, and other large `Int`
values in lambda parameters or `let` bindings, not in buffer cells. Closures
capture ordinary first-class values by value, but `Buf` and `I64Vec` handles
are region-limited: direct `rec` helpers may use them from the surrounding
scope, while first-class closures may not capture them. There is no general
heap buffer, hash map, object system, type class, effect handler, or
user-defined effect in Tacit-Lite. If a program needs those, write the direct
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
   `Int` values in recursive state, records, `I64Vec`, or local bindings.
5. Replace standard-library parsing and formatting with `@parse-i64` and
   `@fmt-i64`. Do not hand-roll those unless the program is specifically about
   parsing or formatting internals.
6. Use records for small named bundles and `@map`, `@fold`, or `@for-each`
   for straight-line `I64Vec` traversal before writing a custom recursive
   loop.
7. Check the effect story last. If the program reads or writes, it has `IO`.
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

### Helper, Closure, And Callback Shapes

Use the helper shape that matches the value being carried. Direct `let`
helpers are ordinary function values. `rec` helpers are for self-recursion or
mutual recursion. First-class closures may capture ordinary values such as
integers, booleans, records, and functions.

```tacit
let base = 40 in
let add_base = lambda x. @add x base in
add_base 2
```

Closures can be returned and then called through a local function value:

```tacit
let make_adder = lambda base. lambda x. @add x base in
let add_ten = make_adder 10 in
add_ten 32
```

Function values can also be stored in records and projected before calling:

```tacit
let ops = {bump: lambda x. @add x 1, double: lambda x. @mul x 2} in
ops.bump 41
```

`Buf` and `I64Vec` handles are the important exception. They are
region-limited handles and cannot be captured by first-class closures:

```tacit fail=invalid-capture
let buf = @buf-alloc 1 in
let get = lambda i. @buf-get buf i in
get 0
```

When a helper needs a buffer or vector from the surrounding expression, make
it a direct `rec` member instead:

```tacit
let buf = @buf-alloc 1 in
rec {get = lambda i. @buf-get buf i} in get 0
```

This direct `rec` shape may use earlier runtime values such as `buf`, `xs`,
or `n` through the compiler-managed direct-call path. A first-class closure
must not capture those handles or store them for later.

Do not bind a whole `rec` group as if it were an ordinary expression:

```text
let skip = rec {skip = lambda i. ...} in skip in ...
```

Put `skip` in the same `rec` group as the helpers that call it, or write a
non-recursive closure if the helper does not need recursion.

Sibling helpers in the same `rec` group do not share parameter scopes. If
`bubble = lambda pass. ... inner ...` and `inner = lambda i. ...`, the name
`pass` is not visible inside `inner`. You must pass `pass` as an explicit
parameter:

```text
rec {
  bubble = lambda pass. lambda i. ... inner pass i ...;
  inner = lambda pass. lambda i. ... pass ...
} in bubble initial_pass 0
```

Closures are useful when the captured value is stable for the lifetime of the
function value. For changing loop state, pass the state explicitly so the
recursive call site remains inspectable:

```text
rec {outer = lambda i.
  ... outer next_i ...
} in outer 0
```

If a nested helper also recurses, lift it into the same `rec` group and pass
the changing outer state explicitly:

```text
rec {
  emit = lambda i. lambda j. ... emit i next_j;
  outer = lambda i. let _ = emit i 0 in outer next_i
} in outer 0
```

For token parsers, this usually means one `rec` group containing `skip`,
`tok_end`, `count`, `process`, and `emit`. If a helper needs `buf`, `n`,
`line_end`, a pattern length, or a mode flag, either make it a `rec` member
that reads the earlier binding directly, or add the value as an explicit
parameter. Do not hide that value inside a returned lambda.

Partial application is now a real function value. It is safe when the captured
values are first-class and escapable:

```tacit
let add = lambda x. lambda y. @add x y in
let add_five = add 5 in
add_five 37
```

Avoid partially applying a `rec` member whose hidden context includes `Buf` or
`I64Vec`; call it directly with all arguments instead. If a branch chooses
behavior, either select between same-typed function values that capture only
first-class data, or pass a mode flag such as `want_even`, `ascending`, or
`emit_separator` and branch inside the helper body.

### Branch Syntax Traps

Every `if` is an expression and must have both `then` and `else`, and each
branch must be a complete expression. A parse error near `if` is almost always
a missing `then`, a missing `else`, a branch that ends early, or a compound
branch that was not parenthesized. There is no brace block syntax. The
expression immediately after `then` must be an atom or application; if the
then-branch begins with `let`, `if`, `rec`, `match`, or `lambda`, wrap that
whole branch in parentheses. The `else` branch may be a
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
`then` branch too narrowly and later reports `expected 'else'` at a much later
token, often a `}` that closes the surrounding `rec`. The error location is
not where the missing parentheses live; the offending `then` is somewhere
above it.

Wrong shape, expanded:

```text
if @eq b 10 then
  let _ = @buf-set out o b in
  process (@add i 1) (@add o 1) 1
else if @eq b 32 then
  ...
```

Both `then` branches start with `let` and are not parenthesized. The parser
treats only the first atom as the branch, then misaligns every subsequent
`else`. Wrap each compound `then`:

```text
if @eq b 10 then
  (let _ = @buf-set out o b in
   process (@add i 1) (@add o 1) 1)
else (if @eq b 32 then
  (let _ = @buf-set out o b in
   process (@add i 1) (@add o 1) 1)
else ...)
```

Safer branch rule: when either side is compound, parenthesize both sides.

```text
if cond then
  (let x = value in result)
else
  (if other then a else b)
```

Do not write `else if` as though it were a separate keyword. Use an explicit
nested expression and parenthesize the inner `if`:

```text
if a then x else (if b then y else z)
```

When a `rec` member contains nested conditionals, prefer parentheses even when
the branch looks short. The semicolon after a `rec` member ends that member;
it does not repair a missing `else`, missing `in`, or too-wide branch.

### Choosing Between `if` And `match`

Use `if` when there is one condition and two outcomes. Use `match` when the
branches correspond to values or constructors. In small programming exercises,
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
use `I64Vec` storage when it is available. For tiny programs, rescanning the
input or carrying the few needed integer values in recursive state can still
be simpler.

Fresh buffers are not guaranteed to contain zeroes. If you later read a cell,
write that cell first. This is especially important for flags such as
`seen`, `used`, `visited`, and `order`: initialize the byte range you will
inspect before the main algorithm. If initialization would be large, avoid
read-before-write by carrying a `first` flag, a logical length, or an explicit
sentinel in recursive state.

Avoid giant stack-sized buffers. A buffer such as `@buf-alloc 16777216` can
crash before the algorithm starts. Prefer the smallest practical bound for the
input shape: 32 bytes for one formatted integer, a few thousand bytes for tiny
examples, 65536 bytes for many line-oriented programs, and at most about one
megabyte unless the input contract clearly requires more. Dynamic allocation is
still local scratch storage, so it is not a reason to allocate hundreds of
megabytes.

For recursive scans over a buffer, allocate the buffer outside the `rec` group
and refer to that buffer by name inside the helper. Use source-level helper
parameters for changing integer state: offsets, lengths, counters, flags, and
accumulators. Do not make the buffer itself a lambda parameter in a recursive
helper. A source-level helper parameter is an integer parameter; shapes such
as `lambda mat. @buf-get mat i` are not the right executable pattern. If you
need the same logic for two buffers, duplicate the small helper or use
separate helpers named for each buffer.

```text
let buf = @buf-alloc-dyn n in
rec {scan = lambda i. lambda acc. ... @buf-get buf i ... scan next_i next_acc} in scan 0 0
```

When a buffer must store indexes or offsets, store a fixed-width byte
encoding, not the raw integer. For inputs below one million bytes, three cells
are enough for an index. Allocate `3 * max_items` cells; this small example
stores up to 1000 indexes:

```text
let idx = @buf-alloc 3000 in
rec {
  set_idx = lambda pos. lambda val.
    let off = @mul pos 3 in
    let _ = @buf-set idx off (@mod val 256) in
    let _ = @buf-set idx (@add off 1) (@mod (@div val 256) 256) in
    @buf-set idx (@add off 2) (@div val 65536);
  get_idx = lambda pos.
    let off = @mul pos 3 in
    @add (@buf-get idx off)
      (@add (@mul (@buf-get idx (@add off 1)) 256)
            (@mul (@buf-get idx (@add off 2)) 65536))
} in ...
```

Use a separate concrete pair such as `set_aux`/`get_aux` for a second buffer.
Do not write a generic `get b pos` helper that takes the buffer as a
parameter; executable helpers should close over concrete buffers bound before
the `rec` group.

If you need to output a slice that starts at a nonzero offset, do not overwrite
the input buffer while later scans still need it. Either copy the slice into a
separate output buffer with `@buf-copy out 0 input start len` and then write
`out len`, or emit one byte at a time through a one-byte scratch buffer.

### Stack And Buffer Safety

The typechecker does not catch out-of-range buffer or range-table access, and
allocation primitives reserve stack space. The following rules prevent the
most common runtime failures:

- Do not allocate multi-megabyte buffers or `I64Vec`s. A call such as
  `@buf-alloc 16777216` or `@i64-alloc 16777216` can crash before the program
  runs. Pick the smallest size that fits the input contract: 32 bytes for one
  formatted integer, a few thousand bytes for tiny inputs, 65536 bytes for
  most line-oriented inputs, and at most about one megabyte unless the input
  contract clearly requires more.
- For a range table built by `@line-index`, `@token-index`, or
  `@token-index-any`, allocate two `I64Vec` slots per possible row. For
  counted range groups built by `@count-equal-ranges`, allocate three slots
  per possible output row. The text length is a safe upper bound on the
  number of rows.
- Treat the integer returned by `@line-index`, `@token-index`,
  `@token-index-any`, `@count-equal-ranges`, and `@dedup-adjacent-ranges` as
  the authoritative row count. Use it as the upper bound for every loop that
  calls `@range-start` or `@range-len`. Do not infer the row count from
  buffer size or table capacity.
- When the returned row count is `0`, do not read row `0`. Branch on the
  count first and return the empty-input result without touching the table.
- Do not call `@buf-copy`, `@buf-eq`, `@parse-i64`, `@range-start`, or
  `@range-len` on offsets or rows that have not first been bounded by the
  relevant length or count. The program must enforce the bound itself.

When a program segfaults, do not change the algorithm first. Reduce allocation
size, add zero-count guards before reading row `0`, and verify that every
table or range access is bounded by the count returned from the indexing
primitive. Bounds discipline is what usually fails, not the algorithm.

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
`cnt`, `parity`, or another changing value must become a sibling and receive
that value as a parameter. This keeps parsing simple, keeps every changing
part visible at the call site, and keeps recursive control flow inspectable.

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
pass it as `loop i acc best flag` or as a sibling helper argument. This
outer-value rule is for `rec` members. A non-recursive `let name = lambda ...`
helper should be closed and should not read `buf`, `n`, or other runtime
values from the surrounding expression.

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

Records are named-field product values. Use them when a function naturally
returns a small bundle or when a short sequence is clearer with named state
than with positional integer parameters.

```tacit
let state = {sum: 6, count: 3} in
@add state.sum state.count
```

Field order is not semantic. `{sum: 6, count: 3}` and `{count: 3, sum: 6}`
have the same structural type. Projection is exact: `state.sum` typechecks
only when the inferred record type contains a `sum` field. There is no record
width subtyping, no row polymorphism, and no record pattern syntax in
Tacit-Lite Phase 4. Destructure with projection:

```tacit
let result = {value: 40, next: 2} in
let value = result.value in
let next = result.next in
@add value next
```

Record fields can hold ordinary first-class values, including function values,
when those values are themselves escapable:

```tacit
let ops = {inc: lambda x. @add x 1, dec: lambda x. @sub x 1} in
ops.inc 41
```

Do not store `Buf` or `I64Vec` handles in records or capture them in closures.
Keep those handles in direct `let` or `rec` scope and pass ordinary integers
or records of integers across function-value boundaries.

### Constructors And Patterns

Capitalized identifiers are constructors. `True` and `False` are known
nullary boolean constructors. Other constructors can appear in parsed syntax
and patterns, but executable programs should usually avoid algebraic data
construction at runtime because the executable subset is intentionally small.
For integer-heavy programs, prefer `match` with integer patterns or `if` with
comparison primitives.

### Primitive Surface

Arithmetic: `@add`, `@sub`, `@mul`, `@div`, `@mod`.

Comparison: `@eq`, `@ne`, `@lt`, `@le`, `@gt`, `@ge`.

IO: `@read`, `@write`, `@exit`.

Allocation: `@buf-alloc`, `@buf-alloc-dyn`, `@i64-alloc`.

Buffer mutation and inspection: `@buf-get`, `@buf-set`, `@buf-copy`,
`@buf-eq`, `@scan-byte`. Note that `@buf-eq buf1 off1 buf2 off2 len` returns
`1` if the byte spans are equal and `0` if they differ. Always branch on the
result directly: `if @buf-eq buf1 o1 buf2 o2 len then equal_branch else
unequal_branch`. Do not invert the return value with `if @eq (@buf-eq ...) 0
then ...`, as this inverts the meaning.

Integer vector storage: `@i64-get`, `@i64-set`, `@i64-swap`, `@i64-copy`.

Higher-order integer-vector traversal: `@map`, `@fold`, `@for-each`.

Text range indexing: `@line-index`, `@token-index`, `@token-index-any`,
`@range-start`, `@range-len`.

Ordering: `@sort-i64`, `@sort-ranges-by-bytes`,
`@stable-sort-pairs-i64`.

Search and range grouping: `@lower-bound-i64`, `@count-equal-ranges`,
`@dedup-adjacent-ranges`.

Parsing and formatting: `@parse-i64`, `@fmt-i64`.

Only the names listed above are recognized primitives. Always keep the leading
`@`. An `@`-prefixed name not in this list will fail typechecking with an
unknown-primitive error; an unprefixed name is treated as a local variable, not
a primitive. If a needed operation is not in this list, build it from the
listed primitives rather than guessing a new name.

The primitive call shape is part of the language contract. For example,
`@buf-copy dst dst_off src src_off len` mutates `dst` and returns an `Int`;
`@buf-eq a a_off b b_off len` is pure and returns an `Int` flag.
`@line-index text len table` and
`@token-index text off len delim table` mutate `table` and return the number
of rows written. `@token-index-any text off len delims delim_count table`
does the same with a delimiter set.
`@sort-i64 xs count`, `@sort-ranges-by-bytes text table count`, and
`@stable-sort-pairs-i64 keys values count` sort in place and return `0`.
`@lower-bound-i64 xs count value` searches a sorted integer vector prefix.
`@count-equal-ranges text table count out` and
`@dedup-adjacent-ranges text table count out` scan adjacent equal byte ranges
and write grouped rows to `out`.

### Higher-Order Combinators

`@map`, `@fold`, and `@for-each` traverse an `I64Vec` prefix. They do not
work on `Buf`, strings, records, or general lists. The count is separate from
the handle, and the visited range is `0 .. count - 1`.

```tacit
let xs = @i64-alloc 3 in
let _ = @i64-set xs 0 1 in
let _ = @i64-set xs 1 2 in
let _ = @i64-set xs 2 3 in
let ys = @i64-alloc 3 in
let offset = 10 in
let _ = @map xs 3 (lambda x. @add x offset) ys in
@fold ys 3 0 (lambda acc. lambda x. @add acc x)
```

`@map xs count f out` calls `f` for each integer element and writes the
integer result into `out` at the same index. `@map` has `Mut` because it
writes `out`; it returns `0`.

`@fold xs count init f` calls `f acc elem` for each element and returns the
final accumulator. The callback is accumulator-first:

```text
@fold xs count 0 (lambda acc. lambda x. @add acc x)
```

`@for-each xs count f` calls `f elem`, ignores the callback result, and
returns `0`. Use it when the callback is effectful:

```text
@for-each xs count (lambda x.
  let out = @buf-alloc 2 in
  let _ = @buf-set out 0 x in
  let _ = @buf-set out 1 10 in
  @write 1 out 2)
```

Combinator callbacks may capture first-class values such as `offset` above.
They must not capture the `I64Vec` or `Buf` handles themselves. If a callback
needs indexed storage beyond the current element, use a direct recursive
helper instead of a combinator.

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

Partial application matters. `@write 1` is a pure function value waiting for
the buffer and length. The `IO` effect appears only at the fully applied call.
Binding or passing a partial value is valid when the resulting closure captures
only first-class data, but for effectful primitives it is usually clearer to
call the primitive with all of its arguments at the point where the work
should happen.

`let` joins the effect of its right-hand side with the effect of its body.
`if` joins the condition, then branch, and else branch. `match` joins the
scrutinee and every arm body. A lambda expression itself is pure to create;
the effect is attached to the function call. A recursive function's call
effect includes `Div` because Tacit-Lite does not prove recursion terminates.
Callbacks contribute the effects of their bodies when a combinator calls
them. `@map` also contributes `Mut` because it writes the output vector.
`@for-each` is the usual shape for effectful callbacks whose integer result is
not important.

### Common Effect Predictions

`@add 1 2`: `{}`.

`@div 10 2`: `{Div}`.

`let b = @buf-alloc 1 in 0`: `{Alloc}`.

`let b = @buf-alloc 1 in @buf-set b 0 7`: `{Alloc, Mut}`.

`let b = @buf-alloc 1 in @read 0 b 1`: `{Alloc, IO, Mut}`.

`let b = @buf-alloc 32 in @fmt-i64 b 0 42`: `{Alloc, Mut}`.

`let xs = @i64-alloc 2 in @i64-set xs 0 7`: `{Alloc, Mut}`.

`let xs = @i64-alloc 1 in @fold xs 1 0 (lambda acc. lambda x. @add acc x)`:
`{Alloc}`.

`let xs = @i64-alloc 1 in let ys = @i64-alloc 1 in @map xs 1 (lambda x. x) ys`:
`{Alloc, Mut}`.

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
it. `Mut` usually comes from `@buf-set`, `@buf-copy`, `@fmt-i64`, `@read`,
`@i64-set`, `@i64-copy`, `@line-index`, `@token-index`,
`@token-index-any`, `@sort-i64`, `@sort-ranges-by-bytes`, or
`@stable-sort-pairs-i64`, and from `@map` writing its output vector. `IO`
comes from `@read`, `@write`, `@exit`, or an effectful callback called by a
combinator. `Alloc` comes from allocation primitives. `Div` comes from
recursion, division, or modulo.

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

Capturing a region-limited buffer in a first-class closure:

```tacit fail=invalid-capture
let buf = @buf-alloc 1 in
let get = lambda i. @buf-get buf i in
get 0
```

Diagnostic kind: `invalid-capture`. Fix: keep buffer access in a direct
helper.

```tacit
let buf = @buf-alloc 1 in
rec {get = lambda i. @buf-get buf i} in get 0
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

`invalid-projection`: a projected field is not present on the inferred record
type. Check the record construction and field spelling.

`apply-non-function`: an application tried to call a value whose type is not a
function. Add the missing function expression or remove the extra argument.

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

`invalid-capture`: a first-class closure captures a non-escapable value such
as `Buf` or `I64Vec`. Use a direct `rec` helper or pass ordinary integer
state instead.

`callback-type-mismatch`: a combinator callback does not have the required
integer function shape. `@map` and `@for-each` expect `lambda x. ...`;
`@fold` expects `lambda acc. lambda x. ...`.

`callback-effect-mismatch`: a `@fold` callback performs effects during the
first curried application. Put effects in the final element-consuming body.

`invalid-accumulator-shape`: a `@fold` accumulator is not an `Int`.

`unsupported-collection-shape`: a combinator collection argument is not an
`I64Vec`.

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
5. Closure capture failure: move `Buf` or `I64Vec` work into a direct `rec`
   helper, or pass only ordinary first-class state into the closure.
6. Combinator callback failure: check the callback arity and accumulator
   order.
7. Effect violation: update the boundary effect set to match source behavior.
8. Unsupported executable shape: rewrite toward the executable subset:
   integer result, direct helper calls, `rec`, `if`, `match`, `let`, records,
   closures over first-class values, combinators, and supported primitives.
9. Wrong output after compile success: keep the type/effect shape and debug
   the algorithm with smaller input.

### Phase 4 Worked Examples

Record state can make an accumulator's meaning visible without positional
conventions:

```tacit
let start = {sum: 0, count: 0} in
let one = {sum: @add start.sum 1, count: @add start.count 1} in
let two = {sum: @add one.sum 2, count: @add one.count 1} in
@add two.sum two.count
```

Returned closures are useful when one value configures later calls:

```tacit
let make_adder = lambda base. lambda x. @add x base in
let add_ten = make_adder 10 in
add_ten 32
```

Combinators remove boilerplate when the traversal is exactly over an
`I64Vec` prefix:

```tacit
let xs = @i64-alloc 3 in
let _ = @i64-set xs 0 1 in
let _ = @i64-set xs 1 2 in
let _ = @i64-set xs 2 3 in
let ys = @i64-alloc 3 in
let _ = @map xs 3 (lambda x. @add x 1) ys in
@fold ys 3 0 (lambda acc. lambda x. @add acc x)
```

### Common Programming Recipes

These recipes are language-level patterns. They name reusable shapes for
small Tacit programs without assuming any external file layout.

Summation over a known range: write a recursive helper with an index and an
accumulator. The condition checks whether the index has reached the bound.
The recursive call advances the index and accumulator. If the bound is known
at authoring time and tiny, a fixed chain of `@add` calls is sometimes
clearer, but for input-sized work use recursion.

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

Searching for one byte span inside another: keep the pattern as `(pat_off,
pat_len)` and the candidate span as `(span_off, span_len)`. A candidate
position `p` is an absolute input offset; compare with
`@buf-eq input pat_off input p pat_len`. Do not accidentally compare the
candidate span against itself. The search loop stops when `p + pat_len` is
greater than `span_off + span_len`. A zero-length pattern is found at the
start of every span.

When scanning bytes read from stdin, treat the byte count returned by `@read`
as the exclusive upper bound. A helper with state `i` should inspect
`@buf-get buf i` only while `i < n`; when `i == n`, return the EOF result.

Copying a slice: use `@buf-copy dst dst_off src src_off len`. Remember that
the first buffer is the destination. Many wrong answers reverse `dst` and
`src`; the typechecker cannot catch that because both are `Buf`.

Ordering a fixed tiny buffer: direct buffer reads and writes can be clearer
than a general recursive ordering routine. For integer vectors, prefer
`@sort-i64 xs count` over hand-written nested loops. For line or token ranges,
prefer `@sort-ranges-by-bytes text table count`; it reorders range rows while
leaving the source bytes in place. For numeric keys with attached values,
prefer `@stable-sort-pairs-i64 keys values count` so equal keys keep their
relative order.

After sorting an integer vector, use `@lower-bound-i64 xs count value` for
lookup or insertion-position searches. After sorting line or token ranges, use
`@dedup-adjacent-ranges text table count out` for unique adjacent byte ranges
and `@count-equal-ranges text table count out` when each run also needs a
count.

**Do not sort and then dedup for first-occurrence uniqueness.** When the task
asks for unique lines in input order or first-occurrence counting, sorting
destroys the original order. The first matching row in the sorted view is not
the first occurrence in the input. Instead, scan the input left to right and
track the previously emitted lines explicitly. For ≤1,000 items, a simple
quadratic check ("have I output this one before?") is both correct and fast
enough. For larger inputs or when you have a sorted structure from another
step, remember that `@dedup-adjacent-ranges` works on the sorted copy and
returns unique ranges in sorted order, not in input order.

Only write a custom ordering loop when the comparison is not one of those
forms. Keep helper state concrete: indexes, offsets, lengths, and any
temporary storage should have one clear role.
naturally. If output includes an aggregate such as a count, compute that
aggregate by a separate scan of matching spans before emitting.

For large-output transforms that select, group, or reorder spans, prefer a
simple complete quadratic pass over a clever partial divide-and-conquer shape.
Direct recursion with explicit indexes is easier to finish and repair than a
program that needs lists, heaps, or a dictionary.

Token streams of signed integers: define `skip`, `token_end`, and `parse_at`
helpers over the original input. `skip pos` advances past spaces and
newlines. `token_end pos` stops at space, newline, or EOF. `parse_at p` calls
`@parse-i64 input p (@sub (token_end p) p)`. This handles negative signs
without hand-scanning digits and avoids corrupting full integers in byte
buffers.

Run-based or predicate-based integer output should parse each token once per
pass and emit immediately. Carry `have` and `prev` when the decision depends
on the previous value; carry `first` for separators. For an empty result,
write only the required newline, not a formatted zero. A safe `emit_int`
shape is:

```text
emit_int = lambda v. lambda first.
  let _ = if first then 0 else @write 1 " " 1 in
  let w = @fmt-i64 out 0 v in
  @write 1 out w
```

Then the loop should return the next `first` flag, where `0` means something
has already been emitted. This works for one output stream or for repeated
passes with different predicates. Do not write a separator after every item;
that leaves trailing spaces on one-element and short-final-row cases.

Grouped fixed-width output is a separator problem, not a formatting special
case. Carry `col` and `seen`. For each emitted integer, pass `@eq col 0` as
the `first` flag. After incrementing `col`, if it equals the group width,
write one newline and reset `col` to zero. At EOF, write a final newline only
when at least one value was seen and `col` is not zero. If there are no
groups to emit, do not print anything.

Line span work should keep `(start, length)` pairs into the original input.
Use helpers `line_end pos`, `next_line end`, `span_eq`, `span_lt`, and
`emit_span`. Emit a span through a one-byte scratch buffer when it may start
at a nonzero offset:

```text
emit_span = lambda off. lambda slen. lambda i.
  if @ge i slen then 0 else
    let _ = @buf-set one 0 (@buf-get input (@add off i)) in
    let _ = @write 1 one 1 in
    emit_span off slen (@add i 1)
```

This is the default shape for line-oriented transforms that compare, select,
deduplicate, reorder, reverse, aggregate, or emit parts of the original input.
Do not copy the whole remaining input into a small formatting buffer, and do
not format a byte span as an integer.

If you hand-scan signed integers, maintain `cur`, `acc`, `in_num`, and `neg`.
On a digit byte, set `cur = cur * 10 + (byte - 48)` and mark `in_num = 1`.
On `45` before digits, set `neg = 1`. On any separator, flush only when
`in_num` is true, adding `cur` or `-cur` to `acc`, then reset `cur`,
`in_num`, and `neg`. At EOF, run the same flush once. Do not allocate a huge
input buffer just because the statement gives a large theoretical maximum.

Factor-pair enumeration does not naturally emit sorted output if you print
`i` and `n / i` together. When output order matters, make one pass for the
small side of each pair and a second pass for the large complements. Exclude
duplicates at square roots explicitly. A `first` flag is the safest separator
rule.

Selecting spans from text: keep byte offsets and lengths into the input, not
a packed integer encoding of the span contents. For best-span selection, carry
current and best starts and lengths, such as `cur_start`, `cur_len`,
`best_start`, and `best_len`. On ties, keep or replace the old best according
to the requested tie rule. At the end, copy the selected bytes into an output
buffer or emit the span directly.

Searching ordered token data: count tokens first, then search with a
half-open interval `[lo, hi)`. Stop when `lo >= hi`. Compute `mid = (lo + hi)
/ 2`, parse the value at token `mid`, then recurse into `[mid + 1, hi)` or
`[lo, mid)`. A closed interval is easy to get wrong at the empty range.

Running recurrences over parsed integers: initialize from the first value
when zero is not a valid identity for all inputs. Carry the current state and
best state explicitly. For example, if all values may be negative, do not
initialize a best value to zero unless zero is truly allowed by the requested
behavior.

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
caller asks for an exit code, return that code or call `@exit`.

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
Phase 4 executable subset. Prefer integer results at the program boundary,
buffers or `I64Vec` for explicit storage, records for small named bundles,
closures over first-class values, `let`, `if`, `match`, `lambda`, `rec`,
combinators, and primitive calls.

If an executable rejects a function value, inspect its capture set and call
shape. A closure that captures `Buf` or `I64Vec` should become a direct `rec`
helper. A recursive helper whose changing state is hidden in nested closures
is usually clearer when lifted into one `rec` group with explicit parameters.

If output differs from the expected bytes only in whitespace — repeated
missing or extra spaces, missing colons, or missing or extra trailing
newlines — this is an output formatting bug, not an algorithm bug. Choose one
separator rule (write a separator before every item except the first, or after
every item except the last) and apply it consistently. Use a `first` flag to
suppress the leading separator, and emit a final trailing newline only when
the program produced output.

If a program crashes with no diagnostic, inspect buffer allocation size and
read-before-write. A giant buffer can overflow local storage. A flags buffer
that was never initialized can make every item look already used, which often
leads to offset `-1` and an invalid buffer access.

If output is wrong but the program compiles and typechecks, reason from input
bytes. Most wrong outputs are one of: off-by-one length,
forgetting to flush the last number at EOF, writing the whole output buffer
instead of `w` bytes, mixing destination and source in `@buf-copy`, or using
ASCII byte values without subtracting `48` for digits.

For base conversion, the first digit computed by repeated division is usually
the least significant digit. If the output expects most-significant first,
either fill an output buffer from right to left or do a second pass that emits
the temporary digits in reverse order.

### Runtime Exit Codes

A nonzero process exit usually indicates a runtime fault that the typechecker
could not catch.

Exit `-11` is a segmentation fault. The most common causes, in order of
likelihood, are an out-of-bounds buffer access, an invalid range-table read, an
excessive stack allocation, and unbounded recursion. Before changing the
algorithm: reduce buffer sizes toward the smallest practical bound; add a
zero-count guard before reading row `0` of any range table; verify that loop
bounds use the row count returned by `@line-index`, `@token-index`, or
`@token-index-any`; and check that `@buf-copy`, `@buf-eq`, `@parse-i64`,
`@range-start`, and `@range-len` are only called on offsets and rows that have
already been bounded by the relevant length or count. A multi-megabyte
`@buf-alloc` or `@i64-alloc` can crash before any algorithmic work runs;
prefer bounded sizes sized for the input contract.

Exit `1` with empty stderr usually means the program's final expression
evaluated to a nonzero status, or a runtime path returned an error sentinel
through an early `@exit` or a conditional branch. Inspect what the final
expression returns on the input that triggered the exit, and check any
conditional `@exit` calls or branches that fall through to a nonzero integer.

### Boundary Conditions To Remember

Empty input: `@read` returns zero immediately. Make the EOF branch return the
right identity value: count zero, sum zero, longest length zero, or the
current accumulated value if the last token has no trailing newline. If the
required output for empty input is just a newline, write that newline; do not
format `0` as the output. A test that expects `\n` and receives `0\n` is an
empty-input formatting bug, not an algorithm bug. Make sure separator and
trailing-newline emission still runs the right number of times when no rows or
tokens were produced.

Single element: ordering, deduplication, selection, and prefix-shaped programs
often fail on one-element input when the recursive step assumes a successor
exists.

Trailing newline: line-oriented programs should decide whether a final empty
line counts. Follow the requested behavior, not a generic Python habit.

ASCII digits: byte `48` is digit zero and byte `57` is digit nine. Convert a
digit byte with `@sub byte 48`. Convert back for output either with
`@fmt-i64` for whole integers or by adding `48` for a single digit byte.

UTF-8 text: byte loops are correct for ASCII input. If the required behavior
is character-based and input may include non-ASCII text, do not reverse or
split the middle bytes of a multi-byte sequence. Copy each UTF-8 sequence as a
unit. For character palindrome checks, the last byte is not necessarily the
last character. Move the right pointer left over continuation bytes in
`128..191`, compare the whole byte span for the left and right characters,
then advance by the span lengths.

For stream transforms that need to emit UTF-8 code points in reverse order,
recursive delayed output is often the shortest safe shape: read the first byte
of one code point, read its continuation bytes into one-byte buffers, recurse
to the stream boundary, then write the saved bytes in their original order.
Stop before writing delimiter bytes that should not participate in the
transform, then emit the required delimiter after the recursion finishes. This
avoids allocating an output buffer as large as the input and avoids invalid
negative indexes on empty input.

Negative numbers: `@parse-i64` handles the primitive contract for integer
text. If hand-scanning bytes, account for a leading minus sign explicitly.
Note that `@mod -3 2` returns `-1`, not `1`. For parity tests that work for
both positive and negative integers, prefer `@ne (@mod v 2) 0` for odd and
`@eq (@mod v 2) 0` for even, but test your expected behavior with negative
inputs first, or adjust: `let r = @mod v 2 in let r2 = if @lt r 0 then @add r
2 else r in @eq r2 0` for portable even-number testing.

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

Use this skeleton for integer-output programs. The final `0` is the exit
status, not output data. For programs that output a string or transformed
bytes, compute the output buffer and logical length, then write that buffer
and length directly with `@write`, then end with `0`. Do not format a buffer
as an integer unless the required output is numeric. The final expression must
be `0` regardless of what you write.

### Checking Edge Cases

Use the requested behavior to infer edge cases. For line-oriented input,
handle empty input and trailing newline. For integer input, handle zero, one
value, and negative signs when the equivalent Python or Rust solution would.
For ordered output, preserve duplicates unless deduplication is explicitly
requested. For first-occurrence behavior, do not update the answer after
finding an equal later occurrence.

Because static checks cover syntax, types, effects, and executable-shape
support, most semantic bugs survive until execution. Before finalizing a
program, mentally run it on: empty input, one token, two tokens, already
sorted input, reverse sorted input, duplicated values, and a final token
without newline.

### Choosing A Recursion State

A good recursive state has a small invariant: `i` is the next offset, `acc`
is the running sum, `best` is the longest seen so far, and `first` is a flag
to suppress the leading separator. Avoid states that require reconstructing
history; carry explicit flags like `in_word` or `first` instead.

### Byte-Oriented String Work

Tacit-Lite strings at the primitive boundary are byte sequences. For
algorithmic string work, copy bytes into buffers and operate on integer byte
values. ASCII lowercase letters are `97` through `122`; uppercase letters
are `65` through `90`; space is `32`; newline is `10`.

For ASCII case shifts and classification, prefer `@ascii-tolower`,
`@ascii-toupper`, `@ascii-is-alpha`, `@ascii-is-digit`, and
`@ascii-is-space` over inline byte-range conditionals. Each takes a single
byte value; case shifts pass non-letters through unchanged, and class
checks return 0 or 1.

For UTF-8 code point work, prefer `@utf8-decode buf off` and
`@utf8-encode buf off cp` over manual branching on the leading byte.
`@utf8-decode` returns the codepoint and byte length packed as
`cp * 8 + byte_len`; advance the read offset by `byte_len` to walk forward.
A `byte_len` of 0 signals invalid input. Use `@utf8-len cp` to size an
output buffer ahead of writing. Tacit-Lite has no Unicode normalization
surface.

When constructing output, maintain an output offset across writes. Either
call `@buf-set out off byte` and recurse with `@add off 1`, or call
`@write-range fd buf off len` to emit a contiguous slice in one step. If
writing a multi-byte formatted integer, use the length returned by
`@fmt-i64` to advance the offset or write immediately.

### Algorithm Selection Rules

Prefer the simplest algorithm that fits the primitive surface. For small
input sizes and byte buffers, quadratic algorithms are often acceptable and
clear. A clever asymptotic improvement that needs a hash map, heap, or
general list library is not a win because those abstractions are not
available.

For search, linear scan is the default unless the input is already ordered and
the required behavior depends on that ordering. For grouping, sorted or
stable-order buffer passes are often clearer than inventing a dictionary. For
row/column data, compute row and column offsets manually and keep the
dimension variables named. For table-like dynamic programming, be wary: if the
state table would be large, the explicit Tacit version will be longer because
there is no general collection library.

For row/column integer data, avoid generic helpers that take a buffer
parameter such as `get_val mat idx` or `set_val mat idx value`. Use separate
helpers for each concrete buffer, for example `get_a`, `get_b`, `set_c`, or
inline the few reads and writes. If values can exceed one byte, either rescan
the input to parse the needed value or pack each integer into several bytes
with concrete `get_a`/`set_a` helpers that read a known buffer.

For nested row/column loops, use the bounds literally. If rows are `0..r` and
columns are `0..c`, the outer loop stops at `r` and the inner loop stops at
`c`. Single-row, single-column, and single-cell cases should still run the
body once.

When emitting row/column data in a different traversal order, first determine
the row and column bounds. To emit a column-oriented view of row-oriented
input, scan each row, find the `col`th token within that row, parse it, and
emit with a `first` flag. This avoids storing a full integer table in byte
buffers. For computations that combine rows and columns, parse dimensions
early and carry the loop coordinates and accumulator explicitly, such as `i`,
`j`, `k`, `acc`, `rows`, `inner`, and `cols`. When the inner coordinate
reaches its bound, return the accumulator.

For traversal over a byte grid with visited state, mutate the input buffer
itself when it is safe to destroy the original marks. For ASCII bit grids, the
byte for `1` is `49` and the byte for `0` is `48`. A traversal helper should
stop when the position is out of range or the byte is not the active mark;
otherwise set that byte to the inactive mark and recurse to neighboring
positions. For space-separated grids, horizontal neighbors may be two bytes
away and vertical neighbors are one row stride away, where `stride` is the
first line length plus one.

### Debugging Generated Tacit

If a program fails to parse, reduce the nearest expression. Parentheses are
needed around compound arguments, especially recursive calls and nested
`if`s. A `let` right-hand side extends until `in`, so missing `in` often
makes the parser recover far away from the actual mistake.

If a program typechecks but cannot run as an executable, remove surface
features that are type-level only or outside the current executable subset.
Favor an `Int` final result, explicit storage, records for small bundles,
closures over first-class values, combinators, primitive calls, and direct
helpers for region-limited handles. Do not return records, functions, or
constructors as the final executable result; compute or print the desired
integer/byte result before the final expression.

If a program passes simple cases and fails larger cases, inspect buffer
capacity and output length. `@buf-alloc 32` is enough for one formatted i64,
not for an arbitrary output string. For output that grows with input, allocate
based on an input-size estimate or reuse the input buffer when the
transformation is in-place and safe.

### What Not To Infer

Do not infer features that are intentionally absent. No list syntax, no
implicit string iteration, no Python-style slicing, no automatic stdout
printing, no mutable locals except through explicit storage primitives, no
general source-visible heap allocation, and no hidden operations beyond the
primitives listed here. Compiler-managed closure storage is not a value you
can allocate, inspect, or free.
If the program needs a table, encode the needed state directly with byte
buffers, `I64Vec`, or recursive integer state. If the program needs a string
operation, use byte buffers, range tables, and the primitives listed in this
primer.
If a shorter solution would need a missing abstraction, write the explicit
one.

## Stdlib Appendix: Indexed Storage, Text Ranges, Ordering, Grouping, Stream IO, ASCII, And UTF-8

Use `I64Vec` when a program needs indexed storage for full integer values.
Byte-oriented buffers still handle raw input/output bytes; an `I64Vec` keeps
signed and large values intact. Allocate it with a count, write cells before
reading them, and thread the count separately because the handle does not
store a length.

```tacit
let xs = @i64-alloc 3 in
let _ = @i64-set xs 0 7 in
let _ = @i64-set xs 1 -2 in
let _ = @i64-set xs 2 10 in
@add (@i64-get xs 0) (@i64-get xs 2)
```

The handle is scoped like other allocation handles: allocate it in a `let`,
then use it inside that body. Recursive helpers may read or write the outer
vector directly when the allocation surrounds the `rec`.

```tacit
let n = 3 in
let xs = @i64-alloc n in
rec {fill = lambda i.
  if @eq i n then 0 else
    let _ = @i64-set xs i i in
    fill (@add i 1)
} in fill 0
```

First-class closures and records must not capture or store the `I64Vec`
handle. When traversal is just element-wise, prefer the Phase 4 combinators:
`@map` writes to an explicit output vector, `@fold` returns an integer
accumulator, and `@for-each` is for effectful callbacks whose result is
ignored.

To store paired ranges or other two-column data, use two consecutive slots per
row. For row `i`, the first slot is `2*i` and the second slot is `2*i+1`.
Use `@range-start` and `@range-len` when those two slots represent a byte
range.

```tacit
let ranges = @i64-alloc 4 in
let _ = @i64-set ranges 0 5 in
let _ = @i64-set ranges 1 3 in
let _ = @i64-set ranges 2 12 in
let _ = @i64-set ranges 3 2 in
let start = @range-start ranges 1 in
let len = @range-len ranges 1 in
@add start len
```

To output a vector element, read it into an integer value, format that value
into output bytes, then write the byte count returned by formatting.

`@line-index text len table` scans `text[0..len)` into line ranges. A line
range excludes the LF byte. Empty lines between LF bytes are kept; a final LF
does not add one more empty row. Allocate two `I64Vec` slots per possible
row, then use the returned count as the row bound.

```tacit
let text = @buf-alloc 128 in
let n = @read 0 text 128 in
let lines = @i64-alloc (@mul n 2) in
let line_count = @line-index text n lines in
if @eq line_count 0 then 0 else @range-len lines 0
```

`@token-index-any text off len delims delim_count table` scans
`text[off..off+len)` into non-empty byte runs separated by any byte in
`delims[0..delim_count)`. Leading, trailing, and repeated delimiters are
skipped. Stored starts are absolute byte offsets into `text`, not offsets
relative to `off`. `delims` may be a string literal or a byte buffer. Prefer
this form when spaces, LF, CR, tabs, or other separator bytes may appear
together.

```tacit
let text = @buf-alloc 128 in
let n = @read 0 text 128 in
let words = @i64-alloc (@mul n 2) in
let word_count = @token-index-any text 0 n " \n\r\t" 4 words in
word_count
```

`@token-index text off len delim table` is the one-byte form. Use it when the
input range has exactly one separator byte; `delim` contributes its low byte.

`@sort-i64 xs count` sorts `xs[0..count)` in ascending signed integer order.
It mutates only that prefix; cells at and beyond `count` are untouched. Use it
after parsing numbers into an `I64Vec`, then read or format the sorted cells
normally.

```tacit
let xs = @i64-alloc 3 in
let _ = @i64-set xs 0 9 in
let _ = @i64-set xs 1 -2 in
let _ = @i64-set xs 2 4 in
let _ = @sort-i64 xs 3 in
@i64-get xs 0
```

`@sort-ranges-by-bytes text table count` sorts the first `count` range rows
by the bytes they reference in `text`. It mutates only the row order in
`table`; the source bytes stay in place. When one range is a prefix of
another, the shorter range sorts first.

`@stable-sort-pairs-i64 keys values count` sorts `keys[0..count)` ascending
and applies the same movement to `values[0..count)`. Equal keys keep their
relative order, so attached values with the same key remain in input order.

```tacit
let keys = @i64-alloc 3 in
let _ = @i64-set keys 0 30 in
let _ = @i64-set keys 1 10 in
let _ = @i64-set keys 2 20 in
let values = @i64-alloc 3 in
let _ = @i64-set values 0 100 in
let _ = @i64-set values 1 200 in
let _ = @i64-set values 2 300 in
let _ = @stable-sort-pairs-i64 keys values 3 in
@i64-get values 0
```

After this runs, `keys` holds `10, 20, 30` and `values` holds `200, 300, 100`.
Reading `values 0` returns `200` because `200` was paired with key `10` in
input order.

`@lower-bound-i64 xs count value` returns the first index in a sorted
ascending `I64Vec` prefix where `value` can be inserted without moving earlier
equal values. If every element is less than `value`, it returns `count`.

```tacit
let xs = @i64-alloc 4 in
let _ = @i64-set xs 0 1 in
let _ = @i64-set xs 1 3 in
let _ = @i64-set xs 2 3 in
let _ = @i64-set xs 3 9 in
@lower-bound-i64 xs 4 3
```

`@dedup-adjacent-ranges text table count out` scans adjacent range rows and
writes pairs (start, length): one pair per run of equal bytes. Use it after
`@sort-ranges-by-bytes` when only unique byte ranges are needed. The returned
integer is the number of output rows. `out` may be the same vector as `table`
for in-place compaction.

`@count-equal-ranges text table count out` scans the same adjacent runs and
writes triples (start, length, count): one triple per run of equal bytes.
Allocate three `I64Vec` slots per possible output row and read triple fields
with `@i64-get`.

```tacit
let text = @buf-alloc 128 in
let n = @read 0 text 128 in
let rows = @i64-alloc (@mul n 2) in
let row_count = @token-index-any text 0 n " \n\r\t" 4 rows in
let _ = @sort-ranges-by-bytes text rows row_count in
let grouped = @i64-alloc (@mul row_count 3) in
@count-equal-ranges text rows row_count grouped
```

`@stdin-slurp buf cap` reads stdin into `buf` until EOF or `cap` bytes;
returns bytes written. Prefer it over a one-byte read loop. `@write-range
fd buf off len` takes a `Buf` (not a `Str` literal); it writes
`buf[off..off+len)` to fd `fd` and returns 0. For literal strings such as
`" "`, `"true"`, or `"\n"`, use `@write fd "..." len` with the literal byte
length instead. `@buf-rev buf off len` reverses bytes `buf[off..off+len)` in
place and returns 0.

```tacit
let buf = @buf-alloc 65536 in
let n = @stdin-slurp buf 65536 in
@write-range 1 buf 0 n
```

```tacit
let buf = @buf-alloc 4096 in
let n = @stdin-slurp buf 4096 in
let _ = @buf-rev buf 0 n in
@write-range 1 buf 0 n
```

`@ascii-tolower b` and `@ascii-toupper b` shift an ASCII letter; non-letters
and bytes outside `0..=127` pass through. `@ascii-is-alpha b`,
`@ascii-is-digit b`, and `@ascii-is-space b` return 1 in the class and 0
otherwise. Space class: `9, 10, 11, 12, 13, 32`. There is no
`@ascii-is-vowel`; compose `@ascii-tolower` with a five-branch equality
check when needed.

```tacit
@ascii-tolower 65
```

```tacit
let buf = @buf-alloc 1 in
let _ = @buf-set buf 0 (@ascii-toupper (@buf-get buf 0)) in
@buf-get buf 0
```

```tacit
let buf = @buf-alloc 1 in
let _ = @buf-set buf 0 32 in
let i = 0 in
let b = @buf-get buf i in
if @eq (@ascii-is-space b) 1 then 0 else 1
```

`@utf8-decode buf off` reads one UTF-8 codepoint at `buf[off]` and returns
`cp * 8 + byte_len` (`byte_len` 1 to 4). Unpack with `@div packed 8` and
`@mod packed 8`; `byte_len == 0` marks invalid input. `@utf8-encode buf off
cp` writes `cp` as 1 to 4 UTF-8 bytes and returns the byte count, or 0
without writing for invalid codepoints (negative, above `0x10FFFF`, or in
the surrogate range `0xD800..0xDFFF`). `@utf8-len cp` returns the byte
length without touching memory.

```tacit
let buf = @buf-alloc 64 in
let n = @stdin-slurp buf 64 in
let packed = @utf8-decode buf 0 in
let cp = @div packed 8 in
let len = @mod packed 8 in
@add cp len
```

```tacit
let out = @buf-alloc 4 in
let n = @utf8-encode out 0 128512 in
@write-range 1 out 0 n
```

```tacit
@utf8-len 20013
```
