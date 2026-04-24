import sys
from collections import Counter


def main() -> None:
    line = sys.stdin.readline().rstrip("\n")
    counts = Counter(line)
    for ch in sorted(counts):
        print(f"{ch}:{counts[ch]}")


if __name__ == "__main__":
    main()
