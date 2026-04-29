# Phase 3 carry-over programs

These three programs satisfy the Phase 2 exit criterion 2 (ADR 0046 §3) as
implemented in Phase 3 Stage 3.

| Program | Description | Effects | Expected output |
|---------|-------------|---------|-----------------|
| `sort.tac` | Insertion sort — in-place over a 5-element `{Alloc}` buffer | `{Alloc, IO, Mut}` | `1\n2\n3\n4\n5\n` on stdout, exit 0 |
| `list.tac` | Recursive sum over a conceptual list `[1,2,3,4,5]` | `{}` | exit 15 |
| `sum-numbers.tac` | Read decimal integers from stdin (one per line), print their sum | `{Alloc, IO, Mut}` | sum on stdout followed by newline, exit 0 |

## Compilation constraints

All three programs operate within the Phase 3 codegen constraints:

- All lambdas are closed: rec lambda bodies only capture rec-function bindings
  and integer constants, never outer buffer pointers or dynamic SSA values.
- `sort.tac` and `sum-numbers.tac` allocate buffers inside the function that
  uses them (`main` and `loop` respectively).
- `list.tac` is a pure computation with no buffer allocation.

## sum-numbers encoding

`sum-numbers.tac` reads stdin one byte at a time and encodes `(sum, cur_num)`
as a single `i64` state value: `state = sum * 100000 + cur_num`. This avoids
the need to capture a buffer pointer across recursive calls while still
correctly summing multiple input lines.
