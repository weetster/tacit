# Corpus Task Manifest

Task index for the Phase 3 evaluation corpus. Target: ~60 tasks at Stage 4
freeze. Current count (implemented): **25**.

- `status: done`     — task.md, tests.jsonl, reference.py, reference.rs all
  present and passing the harness.
- `status: planned`  — ID reserved, task not yet implemented.
- `status: retired`  — formerly present; kept here so IDs are never
  renumbered.
- `O/H` — Open (under `tasks/`) vs. **Held-out** (under `sealed/`). The
  held-out convention is per [ADR 0019](../decisions/0019-corpus-idiom-rules.md);
  the sealing mechanism is per [ADR 0020](../decisions/0020-sealing-held-out-in-repo.md).
  Authoritative ID list: [held-out.txt](held-out.txt). Authoritative
  integrity: [sealed-hashes.txt](sealed-hashes.txt) via `corpus-verify-sealed`.

## Arithmetic (001–010)

| ID  | Slug         | Status  | O/H | Notes                              |
|-----|--------------|---------|-----|------------------------------------|
| 001 | sum-to-n     | done    | O   | Closed-form or loop both idiomatic |
| 002 | fibonacci    | done    | **H** | Iterative pair-update              |
| 003 | factorial    | done    | O   | Loop product; N ≤ 20 (i64 range)   |
| 004 | gcd          | done    | O   | Euclidean; `math.gcd` on Python    |
| 005 | is-prime     | done    | O   | Trial division up to sqrt(n)       |
| 006 | digit-sum    | done    | O   |                                    |
| 007 | to-binary    | done    | O   | Integer → binary string            |
| 008 | integer-sqrt | planned | **H** | Floor(sqrt(n)) without floats      |
| 009 | divisors     | done    | O   | Proper divisors, sorted            |
| 010 | power        | done    | O   | Integer exponentiation             |

## Strings (011–020)

| ID  | Slug              | Status  | O/H | Notes                                |
|-----|-------------------|---------|-----|--------------------------------------|
| 011 | reverse-string    | done    | O   | Reverse by Unicode code point        |
| 012 | count-vowels      | done    | O   | ASCII aeiou/AEIOU                    |
| 013 | is-palindrome     | done    | O   | Case-sensitive, whitespace-sensitive |
| 014 | caesar-cipher     | planned | **H** | Shift ASCII letters by K             |
| 015 | title-case        | done    | O   | Capitalize each word                 |
| 016 | longest-word      | done    | O   | Ties: first occurrence               |
| 017 | common-prefix     | done    | O   | Across N lines of input              |
| 018 | run-length-encode | done    | O   |                                      |
| 019 | char-frequency    | planned | **H** | Sorted by char                       |
| 020 | is-anagram        | done    | O   | Two lines; case/whitespace-sensitive |

## Collections (021–030)

| ID  | Slug              | Status  | O/H | Notes                                   |
|-----|-------------------|---------|-----|-----------------------------------------|
| 021 | unique-in-order   | done    | O   | Remove consecutive duplicates           |
| 022 | running-sum       | done    | **H** | Prefix sums                             |
| 023 | flatten-one-level | done    | O   | Input: lines; each line = inner list    |
| 024 | zip-lists         | planned | **H** | Two lines; truncate to shorter          |
| 025 | partition-eo      | done    | O   | Even first line, odd second line (by value) |
| 026 | group-counts      | done    | O   | `word:count` output, sorted by word     |
| 027 | rotate-left       | done    | O   | Rotate K positions; negative K allowed  |
| 028 | chunks            | done    | O   | Fixed-size chunks, last may be shorter  |
| 029 | merge-sorted      | planned | **H** | Two sorted lines → one sorted line      |
| 030 | transpose-matrix  | done    | O   | N×M grid                                |

## Algorithms (031–050)

| ID  | Slug              | Status  | O/H | Notes                                |
|-----|-------------------|---------|-----|--------------------------------------|
| 031 | binary-search     | done    | O   | Distinct sorted input; -1 if absent  |
| 032 | fizzbuzz          | done    | O   | 1..N                                 |
| 033 | two-sum           | done    | O   | Hashmap pass; unique-solution inputs |
| 034 | balanced-parens   | planned | **H** | `()[]{}`                             |
| 035 | bubble-sort       | done    | O   | Ascending                            |
| 036 | quicksort         | planned | O   | Deterministic pivot (middle)         |
| 037 | merge-sort        | planned | O   |                                      |
| 038 | linear-search     | done    | O   | -1 if absent                         |
| 039 | lcm               | planned | **H** | Via GCD                              |
| 040 | josephus          | planned | O   | n people, step k                     |
| 041 | rle-decode        | done    | O   | Inverse of 018                       |
| 042 | valid-sudoku-row  | planned | O   | 9 digits 1–9 or 0; check 1–9 unique  |
| 043 | levenshtein       | planned | **H** | Edit distance                        |
| 044 | count-islands     | planned | O   | 2D grid, 4-connected                 |
| 045 | reverse-words     | done    | O   | Preserve single spaces               |
| 046 | max-subarray      | done    | O   | Kadane's algorithm; non-empty result |
| 047 | stable-sort-pairs | planned | O   | Sort by key, preserve order on ties  |
| 048 | dedup-keep-last   | planned | **H** | Keep last occurrence                 |
| 049 | matrix-multiply   | planned | O   | Integer matrices                     |
| 050 | longest-run       | done    | O   | Longest consecutive-equal subseq len |

## I/O (051–060)

| ID  | Slug              | Status  | O/H | Notes                                  |
|-----|-------------------|---------|-----|----------------------------------------|
| 051 | line-count        | done    | O   | Trailing newline optional              |
| 052 | sum-numbers       | done    | O   | One integer per line                   |
| 053 | tail-n-lines      | planned | **H** | First line = N; then lines             |
| 054 | grep-substring    | done    | O   | First line = pattern                   |
| 055 | sort-lines        | done    | O   | Byte-lexicographic                     |
| 056 | unique-lines      | done    | O   | Preserve first-seen order              |
| 057 | word-count        | done    | O   | Whitespace-delimited tokens            |
| 058 | csv-sum-column    | planned | **H** | First line = column idx; comma-sep     |
| 059 | echo-reverse      | done    | O   | Reverse order of all lines             |
| 060 | longest-line      | done    | O   | By Unicode code-point count; tie=first |

## Summary

- **Implemented**: 42 of ~60 (70%) — open tasks complete per category:
  arithmetic 001/003/004/005/006/007/009/010 (8),
  strings 011/012/013/015/016/017/018/020 (8),
  collections 021/023/025/026/027/028/030 (7, plus sealed 022),
  algorithms 031/032/033/035/038/041/045/046/050 (9, plus sealed 002),
  I/O 051/052/054/055/056/057/059/060 (8).
- **Held-out marked**: 13 of 60 (22%) — 2 currently implemented (002, 022)

Next implementation batch (suggested): implement remaining held-out tasks
to grow the sealed set, or tackle the remaining open planned tasks.
Held-out candidates: 008 integer-sqrt, 014 caesar-cipher, 019 char-frequency,
024 zip-lists, 029 merge-sorted, 034 balanced-parens, 039 lcm, 043 levenshtein,
048 dedup-keep-last, 053 tail-n-lines, 058 csv-sum-column.
Open planned: 036 quicksort, 037 merge-sort, 040 josephus, 042 valid-sudoku-row,
044 count-islands, 047 stable-sort-pairs, 049 matrix-multiply.
