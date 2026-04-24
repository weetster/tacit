# 054 — grep-substring

Filter lines that contain a given substring.

## Input

First line: the pattern string (substring to search for).  
Remaining lines: the text to search.

## Output

All lines from the text (not counting the pattern line) that contain the pattern as a substring, in their original order, one per output line. If no lines match, output nothing.

## Examples

```
fox
the quick brown fox
jumped over the lazy dog
the fox trot
```
→
```
the quick brown fox
the fox trot
```

```
xyz
hello world
goodbye
```
→
```

```

## Constraints

- Pattern length ≤ 1_000 characters.
- Number of text lines ≤ 100_000.
- Line length ≤ 10_000 characters.
