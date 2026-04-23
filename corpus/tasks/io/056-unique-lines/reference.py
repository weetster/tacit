import sys


def main() -> None:
    for line in dict.fromkeys(sys.stdin.read().splitlines()):
        print(line)


if __name__ == "__main__":
    main()
