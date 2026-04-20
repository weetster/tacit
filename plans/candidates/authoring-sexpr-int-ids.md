# Authoring View Candidate: S-Expressions over Integer IDs

**Status:** Draft candidate for Q1 (Phase 0, Stage 1)
**Parent:** [../plans/phase-0-plan.md](../plans/phase-0-plan.md)

A candidate authoring-view format for the Tacit-Lite AST. Designed for token efficiency under cl100k-class tokenizers, where `(`, `)`, and ` <digit>` each tokenize to 1 token.

## Grammar

```
expr  ::= int                     ; DeBruijn variable reference
        | "(" kind expr* ")"      ; node with kind tag + children
        | "#" literal             ; primitive literal
        | "@" sym                 ; interned symbol (field name, ctor name)

kind  ::= int                     ; index into kind table
```

Bare integers are var refs; literal ints get the `#` sigil. This frees the most common token (small ints) for the most common node (variable use).

## Kind table (draft)

| ID | Kind   | Arity | Notes |
|----|--------|-------|-------|
| 0  | lam    | 1     | body |
| 1  | app    | 2     | fn, arg (curried) |
| 2  | let    | 2     | value, body |
| 3  | rec    | N     | mutual binding group; hashes as one atom |
| 4  | if     | 3     | cond, then, else |
| 5  | match  | 1+N   | scrutinee, arms |
| 6  | arm    | 2     | pattern, body |
| 7  | record | 2N    | (sym, val) pairs |
| 8  | proj   | 2     | record, @field |
| 9  | ctor   | 1+N   | @name, args |
| 10 | hole   | 1     | #diag-id (resolved in sidecar) |
| 11 | ann    | 2     | expr, type |

## Examples

```
; identity:  λx. x
(0 0)

; K:  λx. λy. x
(0 (0 1))

; let f = λx. x in f 42
(2 (0 0) (1 0 #42))

; rec { even = λn. ...odd... ; odd = λn. ...even... }
(3 (0 ...) (0 ...))

; { name: "x", age: 30 }
(7 @name #"x" @age #30)

; parse-failure placeholder
(10 #7)
```

## Token notes

- 4-node tree `(0 (0 1))` ≈ 5 tokens. Same in textual form: `(Lam (Lam (Var 1)))` ≈ 11–13.
- String literals and `@symbols` dominate when present — that's a separate axis the BPE-optimized candidate would attack.
- Multi-digit ints split into multiple tokens (e.g. `42` is 1, `123` is often 1, `1234` may be 2). Worth measuring how often DeBruijn depth exceeds 9.

## Open questions to resolve during the prototype

1. Should `app` be variadic (`(1 f x y z)`) instead of curried? Saves parens for n-ary calls but breaks 1:1 with canonical.
2. Symbol interning scope — per-file or global? Affects whether `@field` indexes can themselves be integers.
3. Should the kind table live in-band (header) or assumed by the reader?
