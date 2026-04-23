import sys


def fizzbuzz(i: int) -> str:
    if i % 15 == 0:
        return "FizzBuzz"
    if i % 3 == 0:
        return "Fizz"
    if i % 5 == 0:
        return "Buzz"
    return str(i)


def main() -> None:
    n = int(sys.stdin.readline())
    for i in range(1, n + 1):
        print(fizzbuzz(i))


if __name__ == "__main__":
    main()
