# 023 — flatten-one-level

Flatten a list of lists of integers, one level deep, into a single
sequence.

## Input

Zero or more lines. Each line contains zero or more space-separated
integers and represents one inner list.

## Output

A single line: every integer from every inner list, in original order,
space-separated. If the combined sequence is empty, output a single empty
line.

## Examples

```
1 2 3
4 5
6
```
→
```
1 2 3 4 5 6
```

```
1 2

3
```
→
```
1 2 3
```

The blank line in the second example is an empty inner list and
contributes no elements.

## Constraints

- Total number of integers across all inner lists ≤ 100_000.
- Each value fits in `i64`.
