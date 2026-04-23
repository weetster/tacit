# 003 — factorial

Print `N!` (N factorial) for a non-negative integer N.

## Input

A single line containing a non-negative integer N (0 ≤ N ≤ 20).

## Output

A single line: the value of `N!` as a decimal integer.

## Examples

```
5
```
→
```
120
```

`N = 0` yields `1` (empty product).

## Constraints

- 20! = 2_432_902_008_176_640_000 fits in an `i64` / `u64`.
- Overflow for N > 20 is out of scope; inputs are guaranteed in range.
