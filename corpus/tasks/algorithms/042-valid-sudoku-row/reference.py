import sys


def is_valid_row(digits: list[int]) -> bool:
    filled = [d for d in digits if d != 0]
    return len(filled) == len(set(filled))


def main() -> None:
    digits = [int(t) for t in sys.stdin.readline().split()]
    print("valid" if is_valid_row(digits) else "invalid")


if __name__ == "__main__":
    main()
