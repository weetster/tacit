import sys


def rotate_left(xs: list[int], k: int) -> list[int]:
    if not xs:
        return xs
    k %= len(xs)
    return xs[k:] + xs[:k]


def main() -> None:
    lines = sys.stdin.read().splitlines()
    xs = [int(t) for t in lines[0].split()]
    k = int(lines[1])
    print(" ".join(str(x) for x in rotate_left(xs, k)))


if __name__ == "__main__":
    main()
