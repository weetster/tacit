import sys


def merge(a: list[int], b: list[int]) -> list[int]:
    result: list[int] = []
    i = j = 0
    while i < len(a) and j < len(b):
        if a[i] <= b[j]:
            result.append(a[i])
            i += 1
        else:
            result.append(b[j])
            j += 1
    result.extend(a[i:])
    result.extend(b[j:])
    return result


def merge_sort(xs: list[int]) -> list[int]:
    if len(xs) <= 1:
        return xs
    mid = len(xs) // 2
    return merge(merge_sort(xs[:mid]), merge_sort(xs[mid:]))


def main() -> None:
    xs = [int(t) for t in sys.stdin.readline().split()]
    print(" ".join(str(x) for x in merge_sort(xs)))


if __name__ == "__main__":
    main()
