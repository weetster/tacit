# 053 — tail-n-lines

Output the last N lines of the input.

## Input

- Line 1: N — the number of lines to return (N ≥ 0).
- Remaining lines: the text body.

## Output

The last N lines of the text body, one per output line. If the body has fewer than N lines, output all of them. If N is 0, output nothing.

## Examples

```
2
foo
bar
baz
```
→
```
bar
baz
```

```
5
foo
bar
```
→
```
foo
bar
```

```
0
foo
bar
```
→
```

```

## Constraints

- 0 ≤ N ≤ 100_000.
- Number of body lines ≤ 100_000.
- Line length ≤ 10_000 characters.
