# 009 — divisors

Find all proper divisors of a positive integer N (every divisor d where 1 ≤ d < N), sorted ascending. For N = 1, the proper divisors list is empty.

## Input

A single line containing a positive integer N (1 ≤ N ≤ 10^9).

## Output

A single line: the proper divisors of N in ascending order, space-separated. If there are no proper divisors (N = 1), output an empty line.

## Examples

```
12
```
→
```
1 2 3 4 6
```

```
7
```
→
```
1
```

```
1
```
→
```

```

## Constraints

- 1 ≤ N ≤ 10^9.
- Collect divisors in O(√N) by iterating up to √N.
