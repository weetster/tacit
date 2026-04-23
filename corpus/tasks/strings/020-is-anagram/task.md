# 020 — is-anagram

Determine whether two strings are anagrams of each other. Comparison is
case-sensitive and whitespace-sensitive: every Unicode code point is
significant.

## Input

Two lines, each a string.

## Output

A single line: `true` if line 1 and line 2 contain exactly the same
multiset of characters, otherwise `false`.

## Examples

```
listen
silent
```
→
```
true
```

```
Listen
silent
```
→
```
false
```

```
hello
world
```
→
```
false
```

## Constraints

- Each line ≤ 10_000 characters.
- Inputs may be empty (two empty lines are anagrams).
- Spaces and punctuation count as characters; no normalisation is applied.
