# 019 — char-frequency

Count the frequency of each character in the input string and report
them sorted by character (ascending ASCII order).

## Input

A single line of text (may be empty; trailing newline not counted).

## Output

One line per distinct character that appears in the input, in ascending
ASCII order. Each line has the format `char:count`.

If the input line is empty, produce no output lines.

## Examples

```
abracadabra
```
→
```
a:5
b:2
c:1
d:1
r:2
```

```
aab
```
→
```
a:2
b:1
```

## Constraints

- Input length ≤ 100_000 characters.
- All characters are printable ASCII (0x20–0x7E).
- The newline terminating the input line is not counted.
