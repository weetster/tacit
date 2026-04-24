import sys


def power(base: int, exp: int) -> int:
    result = 1
    for _ in range(exp):
        result *= base
    return result


def main() -> None:
    lines = sys.stdin.read().splitlines()
    base = int(lines[0])
    exp = int(lines[1])
    print(power(base, exp))


if __name__ == "__main__":
    main()
