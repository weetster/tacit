# 014 — caesar-cipher

Apply a Caesar cipher to a string: shift each ASCII letter by K positions,
wrapping within its case (a–z or A–Z). Non-letter characters are unchanged.

## Input

Two lines:
1. An integer K (the shift amount; may be negative or larger than 25).
2. The text to encode (may be empty).

## Output

A single line: the encoded text.

## Examples

```
3
Hello, World!
```
→
```
Khoor, Zruog!
```

```
-1
bca
```
→
```
abz
```

```
0
unchanged
```
→
```
unchanged
```

## Constraints

- -10^9 ≤ K ≤ 10^9.
- Text length ≤ 100_000 characters.
- Only ASCII letters are shifted; all other characters (digits, punctuation,
  spaces) pass through unchanged.
