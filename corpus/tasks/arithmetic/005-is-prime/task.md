# 005 — is-prime

Decide whether a non-negative integer is prime.

## Input

A single line containing a non-negative integer N (0 ≤ N ≤ 10^9).

## Output

A single line: `yes` if N is prime, `no` otherwise.

## Examples

```
7
```
→
```
yes
```

```
9
```
→
```
no
```

`0` and `1` are **not** prime; `2` is.

## Constraints

- N fits in an `i64` / Python `int`.
- Trial division up to `sqrt(N)` is acceptable at these sizes; no
  Miller-Rabin or sieve is required.
