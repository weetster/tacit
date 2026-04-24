# 038 — linear-search

Find the first index of a target value in a list. Return -1 if absent.

## Input

Two lines:

1. Space-separated integers (the list).
2. A single integer: the target value.

## Output

A single line: the zero-based index of the first occurrence of the target, or `-1` if not found.

## Examples

```
3 1 4 1 5 9 2 6
1
```
→
```
1
```

```
1 2 3 4 5
7
```
→
```
-1
```

```
5
5
```
→
```
0
```

## Constraints

- List length ≤ 100_000 elements.
- Values fit in `i64`.
- The list may be empty (output `-1`).
