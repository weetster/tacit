# 026 — group-counts

Count occurrences of each word in the input and output them as `word:count` pairs, sorted lexicographically by word.

## Input

A single line of space-separated words.

## Output

One `word:count` pair per line, sorted lexicographically (byte order) by word. Each pair is formatted as `word:count` with no spaces.

## Examples

```
the cat sat on the mat the cat
```
→
```
cat:2
mat:1
on:1
sat:1
the:3
```

```
a a a
```
→
```
a:3
```

## Constraints

- Line length ≤ 100_000 characters.
- Words contain only lowercase ASCII letters.
- At least one word is present.
