import sys


def longest_run(xs: list[int]) -> int:
    best = 1
    cur = 1
    for i in range(1, len(xs)):
        if xs[i] == xs[i - 1]:
            cur += 1
            if cur > best:
                best = cur
        else:
            cur = 1
    return best


def main() -> None:
    xs = [int(t) for t in sys.stdin.readline().split()]
    print(longest_run(xs))


if __name__ == "__main__":
    main()
