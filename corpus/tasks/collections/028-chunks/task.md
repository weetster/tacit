# 028 — chunks

Split a list of integers into fixed-size chunks. The last chunk may be shorter if the list length is not a multiple of the chunk size.

## Input

Two lines:

1. Space-separated integers (the list).
2. A positive integer K (the chunk size).

## Output

One line per chunk: space-separated integers. If the input list is empty, output nothing (zero lines).

## Examples

```
1 2 3 4 5
2
```
→
```
1 2
3 4
5
```

```
1 2 3
3
```
→
```
1 2 3
```

```
1 2 3 4
2
```
→
```
1 2
3 4
```

## Constraints

- List length ≤ 100_000 elements.
- Values fit in `i64`.
- 1 ≤ K ≤ 10_000.
