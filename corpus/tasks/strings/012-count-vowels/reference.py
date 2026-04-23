import sys


def count_vowels(text: str) -> int:
    return sum(1 for c in text if c in "aeiouAEIOU")


def main() -> None:
    print(count_vowels(sys.stdin.read()))


if __name__ == "__main__":
    main()
