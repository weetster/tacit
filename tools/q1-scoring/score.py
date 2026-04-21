"""Score Q1 authoring-view candidates by token count under cl100k_base."""

import tiktoken

from candidates import ENCODINGS, REFERENCE_PROGRAM


def main() -> None:
    enc = tiktoken.get_encoding("cl100k_base")

    print(f"Reference program:\n{REFERENCE_PROGRAM}\n")
    print(f"Tokenizer: cl100k_base (tiktoken)\n")
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
