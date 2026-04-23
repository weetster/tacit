import sys


def max_subarray(xs: list[int]) -> int:
    best = cur = xs[0]
    for x in xs[1:]:
        cur = max(x, cur + x)
        best = max(best, cur)
    return best


def main() -> None:
    xs = [int(t) for t in sys.stdin.readline().split()]
    print(max_subarray(xs))


if __name__ == "__main__":
    main()
