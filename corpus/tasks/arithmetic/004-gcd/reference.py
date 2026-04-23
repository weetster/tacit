import sys
from math import gcd


def main() -> None:
    a, b = (int(t) for t in sys.stdin.readline().split())
    print(gcd(a, b))


if __name__ == "__main__":
    main()
