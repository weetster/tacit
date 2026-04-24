# 050 — longest-run

Find the length of the longest run of consecutive equal elements in a list.

## Input

A single line of space-separated integers.

## Output

A single line: the length of the longest run of consecutive equal elements. A single element has a run length of 1.

## Examples

```
1 1 2 2 2 3 1 1
```
→
```
3
```

```
1 2 3 4 5
```
→
```
1
```

```
7 7 7 7
```
→
```
4
```

## Constraints

- List length ≤ 100_000 elements.
- Values fit in `i64`.
- At least one element is present.
