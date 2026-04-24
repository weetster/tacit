import sys


def main() -> None:
    pairs = [(int(k), v) for k, v in (line.split(None, 1) for line in sys.stdin.read().splitlines() if line)]
    for key, val in sorted(pairs, key=lambda p: p[0]):
        print(f"{key} {val}")


if __name__ == "__main__":
    main()
