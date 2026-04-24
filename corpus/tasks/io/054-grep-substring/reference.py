import sys


def main() -> None:
    lines = sys.stdin.read().splitlines()
    pattern = lines[0]
    for line in lines[1:]:
        if pattern in line:
            print(line)


if __name__ == "__main__":
    main()
