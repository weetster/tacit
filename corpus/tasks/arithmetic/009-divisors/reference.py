import sys


def proper_divisors(n: int) -> list[int]:
    if n == 1:
        return []
    divs = []
    i = 1
    while i * i <= n:
        if n % i == 0:
            divs.append(i)
            if i != n // i and n // i != n:
                divs.append(n // i)
        i += 1
    divs.sort()
    return divs


def main() -> None:
    n = int(sys.stdin.readline())
    result = proper_divisors(n)
    print(" ".join(str(d) for d in result))


if __name__ == "__main__":
    main()
