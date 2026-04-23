# 057 — word-count

Count whitespace-delimited tokens across all lines of input.

## Input

Zero or more lines of text. A token is a maximal run of non-whitespace
characters. Whitespace is any ASCII whitespace character (space `' '`,
tab `'\t'`, newline `'\n'`, carriage return `'\r'`, form feed `'\f'`,
vertical tab `'\v'`).

## Output

A single line: the total token count as a decimal integer.

## Examples

```
the quick brown fox
jumps over the lazy dog
```
→
```
9
```

```

```
→
```
0
```

Consecutive whitespace separates tokens but does not produce empty ones.

## Constraints

- Total input size ≤ 1 MB.
- ASCII whitespace only.
