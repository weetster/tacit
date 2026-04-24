import sys


def linear_search(xs: list[int], target: int) -> int:
    for i, x in enumerate(xs):
        if x == target:
            return i
    return -1


def main() -> None:
    lines = sys.stdin.read().splitlines()
    xs = [int(t) for t in lines[0].split()] if lines[0].strip() else []
    target = int(lines[1])
    print(linear_search(xs, target))


if __name__ == "__main__":
    main()
