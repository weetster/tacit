import sys


def factorial(n: int) -> int:
    result = 1
    for i in range(2, n + 1):
        result *= i
    return result


def main() -> None:
    n = int(sys.stdin.readline())
    print(factorial(n))


if __name__ == "__main__":
    main()
