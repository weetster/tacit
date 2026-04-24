import sys


def quicksort(xs: list[int]) -> list[int]:
    if len(xs) <= 1:
        return xs
    pivot = xs[len(xs) // 2]
    left = [x for x in xs if x < pivot]
    mid = [x for x in xs if x == pivot]
    right = [x for x in xs if x > pivot]
    return quicksort(left) + mid + quicksort(right)


def main() -> None:
    xs = [int(t) for t in sys.stdin.readline().split()]
    print(" ".join(str(x) for x in quicksort(xs)))


if __name__ == "__main__":
    main()
