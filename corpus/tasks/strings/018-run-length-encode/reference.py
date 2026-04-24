import sys


def rle_encode(s: str) -> str:
    if not s:
        return ""
    parts = []
    count = 1
    for i in range(1, len(s)):
        if s[i] == s[i - 1]:
            count += 1
        else:
            parts.append(f"{count}{s[i - 1]}")
            count = 1
    parts.append(f"{count}{s[-1]}")
    return "".join(parts)


def main() -> None:
    s = sys.stdin.readline().rstrip("\n")
    print(rle_encode(s))


if __name__ == "__main__":
    main()
