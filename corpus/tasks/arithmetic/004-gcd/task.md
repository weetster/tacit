# 004 — gcd

Compute the greatest common divisor of two non-negative integers using the
Euclidean algorithm.

## Input

A single line containing two space-separated non-negative integers A and B.

## Output

A single line: `gcd(A, B)` as a decimal integer. By convention
`gcd(0, 0) = 0` and `gcd(0, n) = gcd(n, 0) = n`.

## Examples

```
12 18
```
→
```
6
```

```
17 13
```
→
```
1
```

## Constraints

- 0 ≤ A, B ≤ 10^18.
- Both inputs and the result fit in a `u64` / Python `int`.
