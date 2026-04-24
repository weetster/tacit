import sys


def rle_decode(s: str) -> str:
    parts = []
    i = 0
    while i < len(s):
        j = i
        while j < len(s) and s[j].isdigit():
            j += 1
        count = int(s[i:j])
        char = s[j]
        parts.append(char * count)
        i = j + 1
    return "".join(parts)


def main() -> None:
    s = sys.stdin.readline().rstrip("\n")
    print(rle_decode(s))


if __name__ == "__main__":
    main()
