import sys


def group_counts(words: list[str]) -> list[tuple[str, int]]:
    counts: dict[str, int] = {}
    for w in words:
        counts[w] = counts.get(w, 0) + 1
    return sorted(counts.items())


def main() -> None:
    words = sys.stdin.readline().split()
    for word, count in group_counts(words):
        print(f"{word}:{count}")


if __name__ == "__main__":
    main()
