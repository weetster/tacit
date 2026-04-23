import sys


def main() -> None:
    text = sys.stdin.read()
    if not text:
        print(0)
        return
    count = text.count("\n")
    if not text.endswith("\n"):
        count += 1
    print(count)


if __name__ == "__main__":
    main()
