# 049 — matrix-multiply

Multiply two integer matrices A (n×m) and B (m×p), producing a result matrix
C (n×p) where C[i][j] = sum of A[i][k] * B[k][j] for k in 0..m.

## Input

- Line 1: three space-separated integers n, m, p.
- Next n lines: matrix A, each row containing m space-separated integers.
- Next m lines: matrix B, each row containing p space-separated integers.

## Output

n lines, each containing p space-separated integers: the product matrix.

## Examples

```
2 3 2
1 2 3
4 5 6
7 8
9 0
1 2
```
→
```
28 14
79 44
```

```
2 2 2
1 0
0 1
5 6
7 8
```
→
```
5 6
7 8
```

```
1 1 1
3
4
```
→
```
12
```

## Constraints

- 1 ≤ n, m, p ≤ 300.
- Values fit in `i64`; products fit in `i64`.
