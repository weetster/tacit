import sys


def main() -> None:
    rows = [line.split() for line in sys.stdin.read().splitlines() if line]
    n = len(rows[0])
    for col in range(n):
        print(" ".join(row[col] for row in rows))


if __name__ == "__main__":
    main()
