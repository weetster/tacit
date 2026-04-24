import sys


def josephus(n: int, k: int) -> int:
    pos = 0
    for i in range(2, n + 1):
        pos = (pos + k) % i
    return pos


def main() -> None:
    lines = sys.stdin.read().splitlines()
    n = int(lines[0])
    k = int(lines[1])
    print(josephus(n, k))


if __name__ == "__main__":
    main()
