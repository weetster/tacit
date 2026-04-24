import sys
import heapq


def main() -> None:
    line1 = sys.stdin.readline().strip()
    line2 = sys.stdin.readline().strip()
    xs = [int(t) for t in line1.split()] if line1 else []
    ys = [int(t) for t in line2.split()] if line2 else []
    merged = list(heapq.merge(xs, ys))
    print(" ".join(str(x) for x in merged) if merged else "")


if __name__ == "__main__":
    main()
