# 055 — sort-lines

Sort input lines in ascending byte-lexicographic order.

## Input

Any number of lines (zero or more).

## Output

The same lines in ascending order, one per line. For valid UTF-8 input, the
Unicode code-point ordering coincides with the byte ordering, so either view
yields the same result.

The trailing newline is always emitted after the last line. Empty input
produces empty output.

## Examples

```
c
b
a
```
→
```
a
b
c
```

## Constraints

- Input size ≤ 16 MiB.
- Sort is case-sensitive: `A` < `B` < `a` < `b`.
- Equal lines preserve their relative order (stable sort), though no test
  relies on this since equal lines are indistinguishable.
