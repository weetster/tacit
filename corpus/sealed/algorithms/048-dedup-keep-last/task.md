# 048 — dedup-keep-last

Remove duplicate integers from a list, keeping only the last occurrence of each value. The output order follows the position of each value's last occurrence.

## Input

A single line of space-separated integers.

## Output

A single line of space-separated integers with duplicates removed, ordered by last-occurrence position (left to right).

## Examples

```
1 2 1 3
```
→
```
2 1 3
```

```
3 1 2 1 3
```
→
```
2 1 3
```

```
1 2 3
```
→
```
1 2 3
```

## Constraints

- 1 ≤ number of integers ≤ 100_000.
- Each integer fits in a signed 64-bit integer.
