import sys


def digit_sum(n: str) -> int:
    return sum(int(c) for c in n)


def main() -> None:
    n = sys.stdin.readline().strip()
    print(digit_sum(n))


if __name__ == "__main__":
    main()
