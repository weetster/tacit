# 018 — run-length-encode

Encode a string with run-length encoding: each run of consecutive identical characters is replaced by the count followed by the character.

## Input

A single line (the string to encode). May be empty.

## Output

A single line: the run-length encoded form. For example, `aabbbcc` → `2a3b2c`. Single-character runs are still written with a count of 1 (e.g., `abc` → `1a1b1c`).

## Examples

```
aabbbcc
```
→
```
2a3b2c
```

```
abc
```
→
```
1a1b1c
```

```
aaaa
```
→
```
4a
```

## Constraints

- Line length ≤ 100_000 characters.
- The string may be empty (output an empty line).
- Characters are arbitrary bytes (printable ASCII is guaranteed).
