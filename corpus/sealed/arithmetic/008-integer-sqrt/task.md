# 008 — integer-sqrt

Compute the integer square root of N: the largest non-negative integer k
such that k² ≤ N.

## Input

A single line containing a non-negative integer N.

## Output

A single line: floor(sqrt(N)), as a decimal integer.

## Examples

```
16
```
→
```
4
```

```
15
```
→
```
3
```

```
0
```
→
```
0
```

## Constraints

- 0 ≤ N ≤ 10^18.
- N fits in a `u64`.
- The result fits in a `u32`.
- Do not use floating-point arithmetic.
