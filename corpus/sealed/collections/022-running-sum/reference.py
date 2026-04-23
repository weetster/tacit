import sys
from itertools import accumulate


def main() -> None:
    line = sys.stdin.readline().strip()
    xs = [int(t) for t in line.split()] if line else []
    print(" ".join(str(x) for x in accumulate(xs)))


if __name__ == "__main__":
    main()
