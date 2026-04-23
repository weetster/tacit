# 011 — reverse-string

Reverse the characters of a line.

## Input

A single line. The line may contain any Unicode scalar values. The trailing
newline is not part of the content to reverse.

## Output

A single line: the input line reversed by Unicode code point (not by UTF-8
byte), followed by a newline.

## Examples

```
hello
```
→
```
olleh
```

```
café
```
→
```
éfac
```

## Constraints

- Line length ≤ 10_000 code points.
