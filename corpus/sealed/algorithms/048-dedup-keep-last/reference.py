import sys


def dedup_keep_last(nums: list[int]) -> list[int]:
    last_pos = {n: i for i, n in enumerate(nums)}
    return [n for i, n in enumerate(nums) if last_pos[n] == i]


def main() -> None:
    line = sys.stdin.read().strip()
    nums = list(map(int, line.split()))
    print(*dedup_keep_last(nums))


if __name__ == "__main__":
    main()
