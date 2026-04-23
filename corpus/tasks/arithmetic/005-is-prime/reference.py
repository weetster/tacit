import sys


def is_prime(n: int) -> bool:
    if n < 2:
        return False
    if n < 4:
        return True
    if n % 2 == 0:
        return False
    i = 3
    while i * i <= n:
        if n % i == 0:
            return False
        i += 2
    return True


def main() -> None:
    n = int(sys.stdin.readline())
    print("yes" if is_prime(n) else "no")


if __name__ == "__main__":
    main()
