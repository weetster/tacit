import sys


def unique_in_order(xs: list[int]) -> list[int]:
    out: list[int] = []
    for x in xs:
        if not out or out[-1] != x:
            out.append(x)
    return out


def main() -> None:
    line = sys.stdin.readline().strip()
    xs = [int(t) for t in line.split()] if line else []
    print(" ".join(str(x) for x in unique_in_order(xs)))


if __name__ == "__main__":
    main()
