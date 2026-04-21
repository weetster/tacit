"""Score Q1 authoring-view candidates by token count under Claude Opus 4.7.

Uses the Anthropic Messages count_tokens endpoint (free — no inference). Each
encoding is wrapped in a single user message; all candidates pay the same
message-envelope overhead, so cross-candidate ratios are meaningful even though
absolute counts include role/structure tokens.

Requires ANTHROPIC_API_KEY in the environment.
"""

import anthropic

from candidates import ENCODINGS, REFERENCE_PROGRAM

MODEL = "claude-opus-4-7"


def count_message_tokens(client: anthropic.Anthropic, text: str) -> int:
    response = client.messages.count_tokens(
        model=MODEL,
        messages=[{"role": "user", "content": text}],
    )
    return response.input_tokens


def main() -> None:
    client = anthropic.Anthropic()

    print(f"Reference program:\n{REFERENCE_PROGRAM}\n")
    print(f"Tokenizer: {MODEL} (Anthropic count_tokens API)\n")

    baseline = count_message_tokens(client, "x")
    print(f"1-char-message baseline (envelope + 1 char): {baseline} tokens\n")

    print(f"{'Encoding':<20} {'Chars':>6} {'Raw':>5} {'Net':>5} {'Ratio':>7}")
    print("-" * 50)

    counts = {name: count_message_tokens(client, text) for name, text in ENCODINGS.items()}
    nets = {name: counts[name] - baseline for name in ENCODINGS}
    best = min(nets.values())

    for name, text in ENCODINGS.items():
        raw = counts[name]
        net = nets[name]
        ratio = net / best if best > 0 else float("inf")
        print(f"{name:<20} {len(text):>6} {raw:>5} {net:>5} {ratio:>6.2f}x")

    print()
    print("Net = raw - baseline. Ratios use Net.")


if __name__ == "__main__":
    main()
