# 043 — levenshtein

Compute the Levenshtein edit distance between two strings.

## Input

- Line 1: string A (may be empty).
- Line 2: string B (may be empty).

## Output

A single integer: the minimum number of single-character insertions, deletions, or substitutions needed to transform A into B.

## Examples

```
kitten
sitting
```
→
```
3
```

```
hello
hello
```
→
```
0
```

```

hello
```
→
```
5
```

## Constraints

- String lengths ≤ 1_000 characters each.
- Strings consist of printable ASCII.
