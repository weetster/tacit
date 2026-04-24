# 042 — valid-sudoku-row

Check whether a sudoku row is valid. A row contains 9 digits, each either 0
(empty cell) or 1–9 (filled cell). The row is valid if no digit 1–9 appears
more than once among the filled cells.

## Input

One line: 9 space-separated integers, each in the range 0–9.

## Output

`valid` if no filled digit repeats; `invalid` otherwise.

## Examples

```
5 3 4 6 7 8 9 1 2
```
→
```
valid
```

```
8 2 2 3 3 4 5 6 7
```
→
```
invalid
```

```
0 0 0 0 0 0 0 0 0
```
→
```
valid
```

## Constraints

- Input is always exactly 9 space-separated integers, each in 0–9.
