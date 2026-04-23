import sys


def main() -> None:
    line = sys.stdin.readline().rstrip("\n")
    print(line[::-1])


if __name__ == "__main__":
    main()
