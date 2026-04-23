import sys


def main() -> None:
    lines = sys.stdin.read().splitlines()
    for line in sorted(lines):
        print(line)


if __name__ == "__main__":
    main()
