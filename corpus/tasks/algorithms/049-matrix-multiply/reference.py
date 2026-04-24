import sys


def matmul(a: list[list[int]], b: list[list[int]], n: int, m: int, p: int) -> list[list[int]]:
    c = [[0] * p for _ in range(n)]
    for i in range(n):
        for k in range(m):
            for j in range(p):
                c[i][j] += a[i][k] * b[k][j]
    return c


def main() -> None:
    data = sys.stdin.read().split()
    idx = 0
    n, m, p = int(data[idx]), int(data[idx + 1]), int(data[idx + 2])
    idx += 3
    a = [[int(data[idx + i * m + k]) for k in range(m)] for i in range(n)]
    idx += n * m
    b = [[int(data[idx + k * p + j]) for j in range(p)] for k in range(m)]
    for row in matmul(a, b, n, m, p):
        print(" ".join(str(x) for x in row))


if __name__ == "__main__":
    main()
