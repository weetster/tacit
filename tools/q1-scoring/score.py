"""Score Q1 authoring-view candidates by token count under cl100k_base."""

import tiktoken

from candidates import SAMPLES


def score_sample(enc: tiktoken.Encoding, name: str, sample: dict) -> None:
    program: str = sample["program"]
    encodings: dict[str, str] = sample["encodings"]

    print(f"=== Sample: {name} ===")
    print(f"Reference program:\n{program}\n")
    print(f"{'Encoding':<20} {'Chars':>6} {'Tokens':>7} {'Ratio':>7}")
    print("-" * 44)

    counts = {k: len(enc.encode(v)) for k, v in encodings.items()}
    best = min(counts.values())
    for k, v in encodings.items():
        n = counts[k]
        print(f"{k:<20} {len(v):>6} {n:>7} {n/best:>6.2f}x")

    print("\nPer-token breakdown:")
    for k, v in encodings.items():
        toks = enc.encode(v)
        decoded = [enc.decode([t]) for t in toks]
        print(f"\n  {k} ({len(toks)} tokens):\n    {decoded}")
    print()


def main() -> None:
    enc = tiktoken.get_encoding("cl100k_base")
    print("Tokenizer: cl100k_base (tiktoken)\n")
    for name, sample in SAMPLES.items():
        score_sample(enc, name, sample)


if __name__ == "__main__":
    main()
