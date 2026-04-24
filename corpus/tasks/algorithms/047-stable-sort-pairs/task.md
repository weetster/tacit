# 047 — stable-sort-pairs

Sort a list of key–value pairs by their integer key in ascending order,
preserving the original relative order of pairs that share the same key
(stable sort).

## Input

One or more lines, each containing an integer key followed by a space and a
string value (the value contains no spaces).

## Output

The same lines, reordered by key ascending, ties in original order.

## Examples

```
3 foo
1 bar
3 baz
2 qux
```
→
```
1 bar
2 qux
3 foo
3 baz
```

```
1 a
1 b
1 c
```
→
```
1 a
1 b
1 c
```

## Constraints

- 1 ≤ N ≤ 100_000 lines.
- Keys fit in `i64`.
- Values are non-empty strings with no whitespace.
