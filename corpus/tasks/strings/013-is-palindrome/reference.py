import sys


def is_palindrome(s: str) -> bool:
    return s == s[::-1]


def main() -> None:
    line = sys.stdin.readline().rstrip("\n")
    print("yes" if is_palindrome(line) else "no")


if __name__ == "__main__":
    main()
