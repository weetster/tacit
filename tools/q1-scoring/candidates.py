"""Shared encodings for Q1 authoring-view candidate scoring.

Mirrors plans/candidates/reference-ast.md. If you change anything here, update
the markdown doc in the same commit.
"""

SAMPLES: dict[str, dict[str, object]] = {
    "21-node": {
        "program": (
            "let id = lambda x. x in\n"
            "  let pair = { fst: id 1, snd: id 2 } in\n"
            "    if pair.fst then pair.snd else 0"
        ),
        "encodings": {
            "sexpr-int-ids": "(2 (0 0) (2 (7 @fst (1 0 #1) @snd (1 0 #2)) (4 (8 0 @fst) (8 0 @snd) #0)))",
            "glyph-prefix": "= \\ 0 = { @fst . 0 #1 @snd . 0 #2 } ? / 0 @fst / 0 @snd #0",
            "bpe-optimized": "let id = lambda x . x in let pair = { fst : id 1 , snd : id 2 } in if pair . fst then pair . snd else 0",
            "bpe-compact": "let id = lambda x. x in let pair = {fst: id 1, snd: id 2} in if pair.fst then pair.snd else 0",
            "bpe-hybrid": "let = lambda 0 in let = { fst : 0 1 , snd : 0 2 } in if 0 . fst then 0 . snd else 0",
        },
    },
    "100-node": {
        "program": (
            "rec {\n"
            "  length  = lambda xs. match xs with | Nil => Zero | Cons h t => Succ (length t),\n"
            "  isEmpty = lambda xs. match xs with | Nil => True | Cons h t => False,\n"
            "  head    = lambda d. lambda xs. match xs with | Nil => d | Cons h t => h\n"
            "} in\n"
            "let xs : List = Cons 1 (Cons 2 (Cons 3 (Cons 4 (Cons 5 Nil)))) in\n"
            "let first : Nat = head Zero xs in\n"
            "let r = { len: length xs, empty: isEmpty xs, first: first } in\n"
            "if r.empty then _ else r.first"
        ),
        "encodings": {
            # sexpr: kinds 0=lam 1=app 2=let 3=rec 4=if 5=match 6=arm 7=record 8=proj 9=ctor 10=hole 11=ann
            "sexpr-int-ids": (
                "(3"
                " (0 (5 0 (6 (9 @Nil) (9 @Zero)) (6 (9 @Cons 0 0) (9 @Succ (1 3 0)))))"
                " (0 (5 0 (6 (9 @Nil) (9 @True)) (6 (9 @Cons 0 0) (9 @False))))"
                " (0 (0 (5 0 (6 (9 @Nil) 2) (6 (9 @Cons 0 0) 1))))"
                " (2 (11 (9 @Cons #1 (9 @Cons #2 (9 @Cons #3 (9 @Cons #4 (9 @Cons #5 (9 @Nil)))))) (9 @List))"
                "    (2 (11 (1 (1 1 (9 @Zero)) 0) (9 @Nat))"
                "       (2 (7 @len (1 4 1) @empty (1 3 1) @first 0)"
                "          (4 (8 0 @empty) (10 #7) (8 0 @first))))))"
            ),
            # glyph: \=lam .=app ==let ?=if :=ann /=proj *=rec |=match >=arm !=ctor _=hole { }=record
            # variadic kinds (*, |, !) use ( ... ) delimiters
            "glyph-prefix": (
                "*("
                " \\ |( 0 > !( @Nil ) !( @Zero ) > !( @Cons 0 0 ) !( @Succ . 3 0 ) )"
                " \\ |( 0 > !( @Nil ) !( @True ) > !( @Cons 0 0 ) !( @False ) )"
                " \\ \\ |( 0 > !( @Nil ) 2 > !( @Cons 0 0 ) 1 )"
                " = : !( @Cons #1 !( @Cons #2 !( @Cons #3 !( @Cons #4 !( @Cons #5 !( @Nil ) ) ) ) ) ) !( @List )"
                " = : . . 1 !( @Zero ) 0 !( @Nat )"
                " = { @len . 4 1 @empty . 3 1 @first 0 }"
                " ? / 0 @empty _ #7 / 0 @first"
                " )"
            ),
            "bpe-optimized": (
                "rec { length = lambda xs . match xs with | Nil => Zero | Cons h t => Succ ( length t ) ;"
                " isEmpty = lambda xs . match xs with | Nil => True | Cons h t => False ;"
                " head = lambda d . lambda xs . match xs with | Nil => d | Cons h t => h } in"
                " let xs : List = Cons 1 ( Cons 2 ( Cons 3 ( Cons 4 ( Cons 5 Nil ) ) ) ) in"
                " let first : Nat = head Zero xs in"
                " let r = { len : length xs , empty : isEmpty xs , first : first } in"
                " if r . empty then _ else r . first"
            ),
            "bpe-compact": (
                "rec {length = lambda xs. match xs with | Nil => Zero | Cons h t => Succ (length t);"
                " isEmpty = lambda xs. match xs with | Nil => True | Cons h t => False;"
                " head = lambda d. lambda xs. match xs with | Nil => d | Cons h t => h} in"
                " let xs: List = Cons 1 (Cons 2 (Cons 3 (Cons 4 (Cons 5 Nil)))) in"
                " let first: Nat = head Zero xs in"
                " let r = {len: length xs, empty: isEmpty xs, first: first} in"
                " if r.empty then _ else r.first"
            ),
            # hybrid: BPE keyword skeleton, DeBruijn ints at var-ref sites, pat-vars stripped
            "bpe-hybrid": (
                "rec { = lambda match 0 with | Nil => Zero | Cons => Succ ( 3 0 ) ;"
                " = lambda match 0 with | Nil => True | Cons => False ;"
                " = lambda lambda match 0 with | Nil => 2 | Cons => 1 } in"
                " let : List = Cons 1 ( Cons 2 ( Cons 3 ( Cons 4 ( Cons 5 Nil ) ) ) ) in"
                " let : Nat = 1 Zero 0 in"
                " let = { len : 4 1 , empty : 3 1 , first : 0 } in"
                " if 0 . empty then _ else 0 . first"
            ),
        },
    },
}

# Back-compat for anything importing the old flat names.
REFERENCE_PROGRAM = SAMPLES["21-node"]["program"]
ENCODINGS = SAMPLES["21-node"]["encodings"]
