# 007 — to-binary

Convert a non-negative integer to its binary string representation (no leading zeros, except for the input 0 which should output `"0"`).

## Input

A single line containing a non-negative integer N.

## Output

A single line: the binary representation of N as a string of `0` and `1` characters, with no `0b` prefix and no leading zeros (except for N = 0).

## Examples

```
10
```
→
```
1010
```

```
0
```
→
```
0
```

```
255
```
→
```
11111111
```

## Constraints

- 0 ≤ N ≤ 10^18 (fits in a `u64`).
