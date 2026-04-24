# 040 — josephus

Solve the Josephus problem: n people stand in a circle numbered 0 through n−1.
Starting from person 0, every k-th person is eliminated. Return the 0-based
index of the last survivor.

## Input

- Line 1: n — number of people (n ≥ 1).
- Line 2: k — step size (k ≥ 1).

## Output

A single line: the 0-based index of the survivor.

## Examples

```
5
2
```
→
```
2
```

```
1
1
```
→
```
0
```

```
6
1
```
→
```
5
```

## Constraints

- 1 ≤ n ≤ 100_000.
- 1 ≤ k ≤ 1_000_000_000.
