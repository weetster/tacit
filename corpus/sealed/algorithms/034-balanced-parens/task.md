# 034 — balanced-parens

Determine whether a string of bracket characters is balanced.

## Input

A single line containing only the characters `(`, `)`, `[`, `]`, `{`, `}`. The line may be empty.

## Output

`true` if every opener is closed by the correct closer in proper nesting order, `false` otherwise.

## Examples

```
{[()()]}
```
→
```
true
```

```
([)]
```
→
```
false
```

```

```
→
```
true
```

```
(
```
→
```
false
```

## Constraints

- Input length ≤ 100_000 characters.
- Characters outside `()[]{}` do not appear.
