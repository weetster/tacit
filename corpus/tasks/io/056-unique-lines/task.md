# 056 — unique-lines

Deduplicate input lines, preserving the order of first occurrence.

## Input

Zero or more lines of text.

## Output

The same lines in their original order, but with every line after its
first occurrence removed.

## Examples

```
apple
banana
apple
cherry
banana
```
→
```
apple
banana
cherry
```

Blank lines count as lines and are deduplicated like any other:

```
a

b

c
```
→
```
a

b
c
```

## Constraints

- Total input size ≤ 1 MB.
- Lines are compared byte-for-byte; no trimming or case folding.
- Trailing newline on the final line is optional on input; the output
  always ends with a newline after the last distinct line.
