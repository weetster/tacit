"""Score Q1 authoring-view candidates by token count under cl100k_base.

Encodings mirror plans/candidates/reference-ast.md. Update both together if
either changes.
"""

import tiktoken

REFERENCE_PROGRAM = (
    "let id = lambda x. x in\n"
    "  let pair = { fst: id 1, snd: id 2 } in\n"
    "    if pair.fst then pair.snd else 0"
)

ENCODINGS = {
    "sexpr-int-ids": "(2 (0 0) (2 (7 @fst (1 0 #1) @snd (1 0 #2)) (4 (8 0 @fst) (8 0 @snd) #0)))",
    "glyph-prefix": "= \\ 0 = { @fst . 0 #1 @snd . 0 #2 } ? / 0 @fst / 0 @snd #0",
    "bpe-optimized": "let id = lambda x . x in let pair = { fst : id 1 , snd : id 2 } in if pair . fst then pair . snd else 0",
    "bpe-compact": "let id = lambda x. x in let pair = {fst: id 1, snd: id 2} in if pair.fst then pair.snd else 0",
    "bpe-hybrid": "let = lambda 0 in let = { fst : 0 1 , snd : 0 2 } in if 0 . fst then 0 . snd else 0",
}


def main() -> None:
    enc = tiktoken.get_encoding("cl100k_base")

    print(f"Reference program:\n{REFERENCE_PROGRAM}\n")
    print(f"{'Encoding':<20} {'Chars':>6} {'Tokens':>7} {'Ratio':>7}")
    print("-" * 44)

    counts = {name: len(enc.encode(text)) for name, text in ENCODINGS.items()}
    best = min(counts.values())

    for name, text in ENCODINGS.items():
        n = counts[name]
        ratio = n / best
        print(f"{name:<20} {len(text):>6} {n:>7} {ratio:>6.2f}x")

    print()
    print("Per-token breakdown (helps spot fragmentation):")
    for name, text in ENCODINGS.items():
        toks = enc.encode(text)
        decoded = [enc.decode([t]) for t in toks]
        print(f"\n  {name} ({len(toks)} tokens):")
        print(f"    {decoded}")


if __name__ == "__main__":
    main()
