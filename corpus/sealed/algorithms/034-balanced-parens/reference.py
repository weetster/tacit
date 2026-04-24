import sys


def is_balanced(s: str) -> bool:
    stack: list[str] = []
    matching = {')': '(', ']': '[', '}': '{'}
    for c in s:
        if c in '([{':
            stack.append(c)
        else:
            if not stack or stack[-1] != matching[c]:
                return False
            stack.pop()
    return len(stack) == 0


def main() -> None:
    lines = sys.stdin.read().splitlines()
    s = lines[0] if lines else ""
    print("true" if is_balanced(s) else "false")


if __name__ == "__main__":
    main()
