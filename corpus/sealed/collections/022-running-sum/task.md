# 022 — running-sum

Output the running (prefix) sum of a sequence of integers.

**Held-out.** Must not appear in primer examples or training material.

## Input

A single line of zero or more space-separated integers.

## Output

A single line of the prefix sums as space-separated integers, same length as
the input sequence. If input is empty, output a single empty line.

## Examples

```
1 2 3 4 5
```
→
```
1 3 6 10 15
```

```
-1 1 -1 1
```
→
```
-1 0 -1 0
```

## Constraints

- Input length ≤ 100_000 tokens.
- All prefix sums fit in `i64`.
