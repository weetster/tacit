import sys


def main() -> None:
    lines = sys.stdin.read().splitlines()
    n = int(lines[0])
    rest = lines[1:]
    for line in (rest[-n:] if n > 0 else []):
        print(line)


if __name__ == "__main__":
    main()
