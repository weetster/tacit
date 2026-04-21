"""Shared encodings for Q1 authoring-view candidate scoring.

Mirrors plans/candidates/reference-ast.md. If you change anything here, update
the markdown doc in the same commit.
"""

REFERENCE_PROGRAM = (
    "let id = lambda x. x in\n"
    "  let pair = { fst: id 1, snd: id 2 } in\n"
    "    if pair.fst then pair.snd else 0"
)

ENCODINGS: dict[str, str] = {
    "sexpr-int-ids": "(2 (0 0) (2 (7 @fst (1 0 #1) @snd (1 0 #2)) (4 (8 0 @fst) (8 0 @snd) #0)))",
    "glyph-prefix": "= \\ 0 = { @fst . 0 #1 @snd . 0 #2 } ? / 0 @fst / 0 @snd #0",
    "bpe-optimized": "let id = lambda x . x in let pair = { fst : id 1 , snd : id 2 } in if pair . fst then pair . snd else 0",
    "bpe-compact": "let id = lambda x. x in let pair = {fst: id 1, snd: id 2} in if pair.fst then pair.snd else 0",
    "bpe-hybrid": "let = lambda 0 in let = { fst : 0 1 , snd : 0 2 } in if 0 . fst then 0 . snd else 0",
}
