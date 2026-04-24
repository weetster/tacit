import sys


def common_prefix(lines: list[str]) -> str:
    if not lines:
        return ""
    prefix = lines[0]
    for line in lines[1:]:
        i = 0
        while i < len(prefix) and i < len(line) and prefix[i] == line[i]:
            i += 1
        prefix = prefix[:i]
        if not prefix:
            break
    return prefix


def main() -> None:
    lines = sys.stdin.read().splitlines()
    print(common_prefix(lines))


if __name__ == "__main__":
    main()
