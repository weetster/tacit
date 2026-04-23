import sys


def sum_to_n(n: int) -> int:
    return n * (n + 1) // 2


def main() -> None:
    n = int(sys.stdin.readline())
    print(sum_to_n(n))


if __name__ == "__main__":
    main()
