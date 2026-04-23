# 051 — line-count

Count the number of lines in the input.

## Input

Any number of lines (zero or more). A line is a sequence of bytes terminated
by `\n`. A final line without a trailing `\n` still counts as a line.

## Output

A single line: the line count as a decimal integer.

## Examples

```
foo
bar
baz
```
→
```
3
```

Empty input:
```
```
→
```
0
```

## Constraints

- Input size ≤ 16 MiB.
