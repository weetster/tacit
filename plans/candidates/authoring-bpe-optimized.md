# Authoring View Candidate: BPE-Optimized Encoding

**Status:** Draft candidate for Q1 (Phase 0, Stage 1)
**Parent:** [../plans/phase-0-plan.md](../plans/phase-0-plan.md)
**Related:** [authoring-sexpr-int-ids.md](authoring-sexpr-int-ids.md), [authoring-glyph-prefix.md](authoring-glyph-prefix.md)

A candidate authoring view that picks its surface syntax to align with the merge tables of the target tokenizer (Q7). The earlier candidates optimize for *structure*; this one optimizes for *what the BPE has already merged for free*. The bet is that a leading-space `let`, `if`, `lambda`, or `match` tokenizes to a single token under cl100k-class vocabularies, and that mimicking common programming-language surface forms wins more tokens than abbreviating into glyphs or integers.

## Grammar

```
expr  ::= ident                      ; DeBruijn name (display) — canonical strips to int
        | "(" expr ")"
        | "lambda" ident "." expr
        | "let" ident "=" expr "in" expr
        | "if" expr "then" expr "else" expr
        | "match" expr "with" arm+
        | expr expr                  ; juxtaposition = app, left-assoc
        | expr "." ident             ; projection
        | "{" field ("," field)* "}"
        | literal | "@" sym | "_"

arm   ::= "|" pattern "=>" expr
field ::= ident ":" expr
```

Variable references in the *authoring* view use display names; the lossless mapping to canonical DeBruijn ints lives in the sidecar metadata. This is the only candidate that doesn't surface DeBruijn directly — a deliberate tradeoff to recover keyword tokens.

## Why this might win

cl100k and most modern BPEs were trained on a corpus dominated by English and mainstream source code. As a consequence:

- ` let`, ` if`, ` then`, ` else`, ` in`, ` match`, ` with`, ` lambda`, ` def`, ` return` are typically **1 token each** when they occur with a leading space.
- ` =>`, ` ->`, ` ==`, ` =`, ` .`, ` ,`, ` :`, ` |`, ` (`, ` )`, ` {`, ` }` are typically 1 token each.
- Common short identifiers (`x`, `y`, `f`, `n`, ` foo`, ` bar`) are 1 token; longer or unusual identifiers fragment.

The integer-ID and glyph candidates assume each `(` and digit is 1 token, which is true — but they ignore that *whole keywords* are also 1 token. A `let`-form expressed as `let x = e1 in e2` may tokenize to ~7 tokens (incl. spaces); the same form as `(2 (0 0) (1 0 #42))` is ~9.

## Examples (token estimates, cl100k, leading-space normalized)

```
; identity:  λx. x
lambda x . x                                 ; ~5 tokens
; vs. int-IDs (0 0) ≈ 3, glyph "\ 0" ≈ 3   ← BPE LOSES on small terms

; let f = λx. x in f 42
let f = lambda x . x in f 42                 ; ~10 tokens
; vs. int-IDs (2 (0 0) (1 0 #42)) ≈ 12     ← BPE WINS

; { name: "x", age: 30 }
{ name : "x" , age : 30 }                    ; ~9 tokens
; vs. int-IDs (7 @name #"x" @age #30) ≈ 11 ← BPE WINS, and string is shared cost

; if c then a else b
if c then a else b                           ; ~6 tokens
; vs. int-IDs (4 0 1 2) ≈ 5                ← BPE LOSES by 1
```

The pattern: BPE wins on **medium-and-larger** expressions where keyword tokens amortize, and loses on **trivially small** expressions where structural tokens are already minimal. The corpus distribution decides which regime dominates.

## Tradeoffs

**Wins**
- Most readable to humans of the three candidates — looks like a real programming language.
- LLMs have seen this surface form thousands of times in pretraining; in-context inference of edits should be more reliable than for a novel s-expr or glyph form (a soft bet, but a real one).
- String/symbol-heavy programs pay no extra structural overhead — the win the int-ID candidate flagged as out of scope.

**Losses**
- Hard coupling to a specific tokenizer family. Switching from cl100k to a future Anthropic tokenizer could swing token counts ±20%.
- Authoring view loses 1:1 visual correspondence with canonical structure: `f x y` in the view is `(app (app f x) y)` in canonical, and the user has to trust the round-trip.
- Display-name layer is now load-bearing for the authoring view (not just advisory), pushing complexity into the sidecar and the canonicalizer.
- Operator precedence and left-associativity become real grammar concerns; the s-expr form sidesteps them entirely.

## Hybrid worth considering

A two-tier authoring view:
- **Outer skeleton** uses BPE-friendly keywords (`let`, `if`, `match`) for the constructs that benefit.
- **Inner leaves** use bare DeBruijn ints for variable references, skipping the display-name indirection.

```
let f = lambda 0 in 0 #42
```

Reads as "let f = lambda (binds var 0). var 0 in (var 0) 42" — keyword skeleton, integer leaves. Captures most of the BPE win while preserving DeBruijn at the use sites. Probably the form worth measuring first.

## Open questions to resolve during the prototype

1. Q7 is not yet decided. Until the target tokenizer is fixed, all token estimates here are provisional — the candidate should be re-evaluated against whichever tokenizer Stage 1 picks, and the keyword set possibly retuned.
2. How robust is the win once you measure the **whole evaluation corpus** rather than hand-picked snippets? The corpus is frozen in Stage 4; this candidate can't be definitively scored before then.
3. Does the LLM-familiarity argument hold up empirically? Worth a small experiment: ask a model to perform identical edits on the same program in all three notations, measure error rates.
4. If a hybrid (keywords + DeBruijn leaves) wins, is it still recognizably "an authoring view," or has it drifted close enough to canonical that the two-views story collapses to one?
