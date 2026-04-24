# 044 — count-islands

Count the number of islands in a 2D grid. An island is a maximal group of
cells with value `1` connected horizontally or vertically (4-connected).

## Input

One or more lines, each containing space-separated `0`s and `1`s. All rows
have the same number of columns.

## Output

A single line: the number of islands.

## Examples

```
1 1 0
0 1 0
0 0 1
```
→
```
2
```

```
1 0 1
0 0 0
1 0 1
```
→
```
4
```

```
0 0 0
0 0 0
```
→
```
0
```

```
1 1 1
1 1 1
1 1 1
```
→
```
1
```

## Constraints

- 1 ≤ rows, cols ≤ 300.
- Each cell is `0` or `1`.
