# 060 — longest-line

Find the longest line in the input, measured by Unicode code-point count. On ties, return the first occurrence.

## Input

One or more lines.

## Output

A single line: the longest line (by code-point count). On ties, the first occurrence.

## Examples

```
hello
hi
hey there
```
→
```
hey there
```

```
abc
def
ghi
```
→
```
abc
```

## Constraints

- 1 ≤ number of lines ≤ 100_000.
- Line length ≤ 10_000 Unicode code points.
