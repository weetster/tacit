# 025 — partition-eo

Partition a sequence of integers into evens and odds by value parity,
preserving original order within each partition.

## Input

A single line of zero or more space-separated integers.

## Output

Two lines:

1. The even-valued integers in input order, space-separated.
2. The odd-valued integers in input order, space-separated.

Either partition may be empty; empty partitions emit an empty line (only the
trailing `\n`).

## Examples

```
1 2 3 4 5
```
→
```
2 4
1 3 5
```

```
2 4 6
```
→
```
2 4 6

```

The second example ends with an empty odds line followed by a trailing
newline.

## Constraints

- Input length ≤ 100_000 integers.
- Each value fits in an `i64`.
- A negative odd number is odd (value parity, not absolute-value parity).
