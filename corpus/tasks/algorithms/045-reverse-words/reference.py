import sys


def main() -> None:
    line = sys.stdin.readline().rstrip("\n")
    words = line.split(" ")
    print(" ".join(reversed(words)))


if __name__ == "__main__":
    main()
