# 035 — bubble-sort

Sort integers in ascending order using the bubble sort algorithm.

## Input

A single line containing zero or more space-separated integers.

## Output

A single line: the integers sorted ascending, space-separated. If the
input is empty, output a single empty line.

## Examples

```
3 1 4 1 5 9 2 6
```
→
```
1 1 2 3 4 5 6 9
```

```
5
```
→
```
5
```

## Constraints

- Input length ≤ 1_000 integers (bubble sort's O(n²) is fine at this
  scale).
- Each value fits in `i64`.
- The sort must be implemented as bubble sort, not delegated to a library
  sort. A swap-flag early-exit is allowed but not required.
