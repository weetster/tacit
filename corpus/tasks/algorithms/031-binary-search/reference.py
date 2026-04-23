import sys
from bisect import bisect_left


def binary_search(xs: list[int], target: int) -> int:
    i = bisect_left(xs, target)
    if i < len(xs) and xs[i] == target:
        return i
    return -1


def main() -> None:
    lines = sys.stdin.read().splitlines()
    xs = [int(t) for t in lines[0].split()]
    target = int(lines[1])
    print(binary_search(xs, target))


if __name__ == "__main__":
    main()
