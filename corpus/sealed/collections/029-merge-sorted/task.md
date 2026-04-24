# 029 — merge-sorted

Merge two sorted sequences of integers into a single sorted sequence.

## Input

Two lines, each containing zero or more space-separated integers in
non-decreasing order.

## Output

A single line: all integers from both sequences merged in non-decreasing
order, space-separated. If both sequences are empty, output a single
empty line.

## Examples

```
1 3 5
2 4 6
```
→
```
1 2 3 4 5 6
```

```
1 2 3

```
→
```
1 2 3
```

```


```
→
```

```

## Constraints

- Each sequence contains at most 100_000 integers.
- Each value fits in an `i64`.
- Each input sequence is already sorted in non-decreasing order.
