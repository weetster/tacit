# 006 — digit-sum

Sum the decimal digits of a non-negative integer.

## Input

A single line containing a non-negative integer N.

## Output

A single line: the sum of the decimal digits of N, as a decimal integer.

## Examples

```
123
```
→
```
6
```

```
0
```
→
```
0
```

```
9999
```
→
```
36
```

## Constraints

- 0 ≤ N ≤ 10^18 (fits in a `u64`; use string-digit iteration, not division loops, to avoid overflow edge cases).
- The digit sum fits in an `i64`.
