# 013 — is-palindrome

Decide whether a string reads the same forwards and backwards. The check is
case-sensitive and whitespace-sensitive (no normalization is applied).
Comparison is by Unicode code point.

## Input

A single line of text. The terminating newline, if any, is not part of the
string being checked.

## Output

`yes` if the input is a palindrome, `no` otherwise. Followed by a single
trailing newline.

## Examples

```
racecar
```
→
```
yes
```

```
Aa
```
→
```
no
```

The empty string is a palindrome.

## Constraints

- Input length ≤ 100_000 code points.
- Input is valid UTF-8.
