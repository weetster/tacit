# 024 — zip-lists

Zip two sequences of integers together, truncating to the length of the
shorter sequence.

## Input

Two lines, each containing zero or more space-separated integers.

## Output

One line per pair, formatted as `a b` where `a` is from the first
sequence and `b` from the second. If either sequence is empty, produce
no output lines.

## Examples

```
1 2 3
4 5 6
```
→
```
1 4
2 5
3 6
```

```
1 2 3 4
10 20
```
→
```
1 10
2 20
```

```
1 2 3

```
→
```

```

(No output lines when the second list is empty.)

## Constraints

- Each sequence contains at most 100_000 integers.
- Each value fits in an `i64`.
