import sys


def title_case(line: str) -> str:
    return " ".join(w.capitalize() for w in line.split(" "))


def main() -> None:
    line = sys.stdin.readline().rstrip("\n")
    print(title_case(line))


if __name__ == "__main__":
    main()
