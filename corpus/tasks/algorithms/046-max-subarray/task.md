# 046 — max-subarray

Maximum contiguous subarray sum (Kadane's algorithm). The subarray must be
non-empty, so the result for an all-negative input is the maximum single
element.

## Input

A single line containing one or more space-separated integers.

## Output

A single line: the maximum sum over all contiguous non-empty subarrays, as
a decimal integer.

## Examples

```
-2 1 -3 4 -1 2 1 -5 4
```
→
```
6
```

(The subarray `4 -1 2 1` sums to 6.)

```
-5 -2 -8 -1
```
→
```
-1
```

```
1 2 3
```
→
```
6
```

## Constraints

- Input length ≤ 100_000 elements.
- Each value fits in `i64` and the answer fits in `i64`.
- Input is guaranteed non-empty.
