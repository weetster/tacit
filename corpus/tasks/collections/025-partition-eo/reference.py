import sys


def main() -> None:
    xs = [int(t) for t in sys.stdin.readline().split()]
    evens = [x for x in xs if x % 2 == 0]
    odds = [x for x in xs if x % 2 != 0]
    print(" ".join(str(x) for x in evens))
    print(" ".join(str(x) for x in odds))


if __name__ == "__main__":
    main()
