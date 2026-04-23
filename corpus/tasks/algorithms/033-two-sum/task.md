# 033 — two-sum

Find two indices `(i, j)` with `i < j` such that `nums[i] + nums[j] == target`.

## Input

Two lines:

1. A single integer `target`.
2. Zero or more space-separated integers, the array `nums`.

## Output

A single line:

- If a valid pair exists: `i j` (two 0-based indices, `i < j`, space-separated).
- Otherwise: `-1`.

## Examples

```
9
2 7 11 15
```
→
```
0 1
```

```
100
1 2 3
```
→
```
-1
```

## Constraints

- `len(nums) ≤ 100_000`.
- Each value and the target fit in an `i64`.
- Inputs are guaranteed to have at most one valid pair; when present, the
  pair is unique so the output is unambiguous.
