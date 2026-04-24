# 041 — rle-decode

Decode a run-length encoded string. This is the inverse of task 018 (rle-encode): given a string of the form `<count><char>...`, expand it into the original string.

## Input

A single line: the run-length encoded string (e.g., `2a3b2c`). Counts are one or more decimal digits immediately followed by a single character. May be empty (empty input → empty output).

## Output

A single line: the decoded string. If the input is empty, output an empty line.

## Examples

```
2a3b2c
```
→
```
aabbbcc
```

```
1a1b1c
```
→
```
abc
```

```
4a
```
→
```
aaaa
```

## Constraints

- Input length ≤ 100_000 characters.
- Each count is a positive integer; the decoded length ≤ 1_000_000 characters.
- Counts are always followed by exactly one character.
