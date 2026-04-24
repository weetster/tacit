import sys


def chunks(xs: list[int], k: int) -> list[list[int]]:
    return [xs[i:i + k] for i in range(0, len(xs), k)]


def main() -> None:
    lines = sys.stdin.read().splitlines()
    xs = [int(t) for t in lines[0].split()] if lines[0].strip() else []
    k = int(lines[1])
    for chunk in chunks(xs, k):
        print(" ".join(str(x) for x in chunk))


if __name__ == "__main__":
    main()
