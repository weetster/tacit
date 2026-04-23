import sys


def main() -> None:
    total = sum(int(line) for line in sys.stdin if line.strip())
    print(total)


if __name__ == "__main__":
    main()
