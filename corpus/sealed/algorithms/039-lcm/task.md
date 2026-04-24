# 039 — lcm

Compute the least common multiple (LCM) of two non-negative integers.

## Input

A single line containing two non-negative integers a and b.

## Output

A single line: LCM(a, b).

## Examples

```
4 6
```
→
```
12
```

```
12 18
```
→
```
36
```

```
0 5
```
→
```
0
```

## Constraints

- 0 ≤ a, b ≤ 10^9.
- Both values fit in a `u64`.
- The result fits in a `u64` (LCM ≤ 10^18).
- LCM(0, x) = LCM(x, 0) = 0 by convention.
