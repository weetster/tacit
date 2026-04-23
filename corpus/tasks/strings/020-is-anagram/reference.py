import sys


def is_anagram(a: str, b: str) -> bool:
    return sorted(a) == sorted(b)


def main() -> None:
    lines = sys.stdin.read().splitlines()
    a = lines[0] if len(lines) > 0 else ""
    b = lines[1] if len(lines) > 1 else ""
    print("true" if is_anagram(a, b) else "false")


if __name__ == "__main__":
    main()
