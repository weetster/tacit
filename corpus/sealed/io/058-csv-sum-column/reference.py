import sys


def main() -> None:
    lines = sys.stdin.read().splitlines()
    col = int(lines[0])
    total = sum(int(line.split(",")[col]) for line in lines[1:] if line)
    print(total)


if __name__ == "__main__":
    main()
