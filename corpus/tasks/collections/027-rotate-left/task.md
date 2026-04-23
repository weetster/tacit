# 027 — rotate-left

Rotate a list of integers left by K positions. Rotating left by one moves
the first element to the end.

## Input

Two lines:

1. Zero or more space-separated integers.
2. A single integer K. K may be negative (rotate right) or larger than the
   list length.

## Output

A single line: the rotated sequence as space-separated integers. If the
input list is empty, output a single empty line.

## Examples

```
1 2 3 4 5
2
```
→
```
3 4 5 1 2
```

```
1 2 3
-1
```
→
```
3 1 2
```

```
1 2 3
7
```
→
```
2 3 1
```

## Constraints

- Input length ≤ 100_000 elements.
- Values fit in `i64`.
- |K| ≤ 10^9.
- For non-empty input of length N, the result is the input rotated by
  `K mod N` positions, using the Python/Rust Euclidean `mod` (always
  non-negative) so negative K rotates right.
