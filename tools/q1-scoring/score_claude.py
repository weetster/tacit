"""Score Q1 authoring-view candidates by token count under Claude Opus 4.7.

Uses the Anthropic Messages count_tokens endpoint (free — no inference). Each
encoding is wrapped in a single user message; all candidates pay the same
message-envelope overhead, so cross-candidate ratios are meaningful even though
absolute counts include role/structure tokens.

Requires ANTHROPIC_API_KEY in the environment.
"""

import anthropic

from candidates import SAMPLES

MODEL = "claude-opus-4-7"


def count_message_tokens(client: anthropic.Anthropic, text: str) -> int:
    response = client.messages.count_tokens(
        model=MODEL,
        messages=[{"role": "user", "content": text}],
    )
    return response.input_tokens


def score_sample(client: anthropic.Anthropic, baseline: int, name: str, sample: dict) -> None:
    program: str = sample["program"]
    encodings: dict[str, str] = sample["encodings"]

    print(f"=== Sample: {name} ===")
    print(f"Reference program:\n{program}\n")
    print(f"{'Encoding':<20} {'Chars':>6} {'Raw':>5} {'Net':>5} {'Ratio':>7}")
    print("-" * 50)

    counts = {k: count_message_tokens(client, v) for k, v in encodings.items()}
    nets = {k: counts[k] - baseline for k in encodings}
    best = min(nets.values())
    for k, v in encodings.items():
        raw, net = counts[k], nets[k]
        ratio = net / best if best > 0 else float("inf")
        print(f"{k:<20} {len(v):>6} {raw:>5} {net:>5} {ratio:>6.2f}x")
    print()


def main() -> None:
    client = anthropic.Anthropic()
    print(f"Tokenizer: {MODEL} (Anthropic count_tokens API)\n")

    baseline = count_message_tokens(client, "x")
    print(f"1-char-message baseline (envelope + 1 char): {baseline} tokens")
    print("Net = raw - baseline. Ratios use Net.\n")

    for name, sample in SAMPLES.items():
        score_sample(client, baseline, name, sample)


if __name__ == "__main__":
    main()
