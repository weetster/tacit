# 017 — common-prefix

Find the longest common prefix across all input lines.

## Input

One or more lines of text. Each line is taken as-is (no trimming).

## Output

A single line: the longest string that is a prefix of every input line. If no common prefix exists (or the input has zero length common prefix), output an empty line.

## Examples

```
flower
flow
flight
```
→
```
fl
```

```
dog
racecar
car
```
→
```

```

```
interview
```
→
```
interview
```

## Constraints

- 1 ≤ number of lines ≤ 1_000.
- Line length ≤ 1_000 characters.
