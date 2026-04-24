import sys


def main() -> None:
    lines = sys.stdin.read().splitlines()
    best = lines[0]
    for line in lines[1:]:
        if len(line) > len(best):
            best = line
    print(best)


if __name__ == "__main__":
    main()
