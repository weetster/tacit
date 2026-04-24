# 030 — transpose-matrix

Transpose a matrix of integers (rows become columns).

## Input

N lines, each containing M space-separated integers. N ≥ 1, M ≥ 1.

## Output

M lines, each containing N space-separated integers: the transposed matrix.

## Examples

```
1 2 3
4 5 6
```
→
```
1 4
2 5
3 6
```

```
1 2
3 4
5 6
```
→
```
1 3 5
2 4 6
```

```
7
```
→
```
7
```

## Constraints

- 1 ≤ N, M ≤ 1_000.
- Values fit in `i64`.
- All rows have the same number of columns.
