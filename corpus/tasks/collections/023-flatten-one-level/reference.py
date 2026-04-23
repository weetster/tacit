import sys


def main() -> None:
    lines = sys.stdin.read().splitlines()
    flat = [int(t) for line in lines for t in line.split()]
    print(" ".join(str(x) for x in flat))


if __name__ == "__main__":
    main()
