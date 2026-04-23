import sys


def two_sum(target: int, nums: list[int]) -> tuple[int, int] | None:
    seen: dict[int, int] = {}
    for i, n in enumerate(nums):
        if (target - n) in seen:
            return (seen[target - n], i)
        seen[n] = i
    return None


def main() -> None:
    target = int(sys.stdin.readline())
    nums = [int(t) for t in sys.stdin.readline().split()]
    result = two_sum(target, nums)
    if result is None:
        print(-1)
    else:
        print(f"{result[0]} {result[1]}")


if __name__ == "__main__":
    main()
