# 015 — title-case

Title-case a line by capitalizing the first ASCII letter of each
space-separated word.

## Input

A single line of ASCII text. Words are the substrings produced by splitting
on single space characters (`' '`), so consecutive spaces yield empty
words.

## Output

A single line: for each word, the first ASCII letter is uppercased and all
subsequent ASCII letters are lowercased. Non-letter characters pass
through unchanged. Words are rejoined with single spaces, preserving the
original number of separators.

## Examples

```
hello world
```
→
```
Hello World
```

```
THE QUICK brown fox
```
→
```
The Quick Brown Fox
```

```
a1b2 c3d4
```
→
```
A1b2 C3d4
```

## Constraints

- Line length ≤ 10_000 characters.
- Input is ASCII; Unicode case handling is out of scope.
- Consecutive spaces produce empty words (whose title-cased form is also
  empty), so the space runs are preserved in the output.
