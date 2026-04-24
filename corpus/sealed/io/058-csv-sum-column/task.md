# 058 — csv-sum-column

Sum all values in a specified column of a CSV table.

## Input

- Line 1: K — the 0-based column index to sum.
- Remaining lines: comma-separated rows of integers. Every row has at least K+1 fields.

## Output

A single integer: the sum of all values in column K.

## Examples

```
1
1,2,3
4,5,6
7,8,9
```
→
```
15
```

```
0
10,20,30
5,5,5
```
→
```
15
```

## Constraints

- 0 ≤ K ≤ 999.
- 1 ≤ number of data rows ≤ 100_000.
- All values are integers that fit in a signed 64-bit integer.
- The sum fits in a signed 64-bit integer.
