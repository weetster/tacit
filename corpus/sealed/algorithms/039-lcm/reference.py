import sys
from math import gcd


def lcm(a: int, b: int) -> int:
    if a == 0 or b == 0:
        return 0
    return a // gcd(a, b) * b


def main() -> None:
    a, b = (int(t) for t in sys.stdin.readline().split())
    print(lcm(a, b))


if __name__ == "__main__":
    main()
