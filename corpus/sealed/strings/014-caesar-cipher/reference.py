import sys


def shift_char(c: str, k: int) -> str:
    if "a" <= c <= "z":
        return chr((ord(c) - ord("a") + k) % 26 + ord("a"))
    if "A" <= c <= "Z":
        return chr((ord(c) - ord("A") + k) % 26 + ord("A"))
    return c


def caesar(text: str, k: int) -> str:
    return "".join(shift_char(c, k) for c in text)


def main() -> None:
    k = int(sys.stdin.readline())
    text = sys.stdin.readline().rstrip("\n")
    print(caesar(text, k))


if __name__ == "__main__":
    main()
