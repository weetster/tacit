import sys


def bubble_sort(xs: list[int]) -> list[int]:
    a = xs[:]
    n = len(a)
    for i in range(n):
        for j in range(n - 1 - i):
            if a[j] > a[j + 1]:
                a[j], a[j + 1] = a[j + 1], a[j]
    return a


def main() -> None:
    line = sys.stdin.readline().strip()
    xs = [int(t) for t in line.split()] if line else []
    print(" ".join(str(x) for x in bubble_sort(xs)))


if __name__ == "__main__":
    main()
