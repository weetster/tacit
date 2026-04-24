import sys


def longest_word(line: str) -> str:
    words = line.split(" ")
    best = words[0]
    for w in words[1:]:
        if len(w) > len(best):
            best = w
    return best


def main() -> None:
    line = sys.stdin.readline().rstrip("\n")
    print(longest_word(line))


if __name__ == "__main__":
    main()
