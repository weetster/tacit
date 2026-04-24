import sys


def to_binary(n: int) -> str:
    if n == 0:
        return "0"
    bits = []
    while n > 0:
        bits.append(str(n & 1))
        n >>= 1
    return "".join(reversed(bits))


def main() -> None:
    n = int(sys.stdin.readline())
    print(to_binary(n))


if __name__ == "__main__":
    main()
