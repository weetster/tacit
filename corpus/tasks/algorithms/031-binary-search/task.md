# 031 — binary-search

Binary search for a target value in a sorted list of distinct integers.

## Input

- Line 1: one or more space-separated integers, sorted ascending, all
  distinct.
- Line 2: a single integer, the target.

## Output

A single line: the 0-based index of the target in line 1, or `-1` if absent.

## Examples

```
1 3 5 7 9 11
7
```
→
```
3
```

```
1 3 5 7 9 11
4
```
→
```
-1
```

## Constraints

- Input line 1 length ≤ 1_000_000 elements.
- All values fit in `i64`.
