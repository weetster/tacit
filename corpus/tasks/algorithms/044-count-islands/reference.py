import sys
from collections import deque


def count_islands(grid: list[list[int]]) -> int:
    rows = len(grid)
    cols = len(grid[0])
    visited = [[False] * cols for _ in range(rows)]
    count = 0
    for r in range(rows):
        for c in range(cols):
            if grid[r][c] == 1 and not visited[r][c]:
                count += 1
                queue: deque[tuple[int, int]] = deque([(r, c)])
                visited[r][c] = True
                while queue:
                    cr, cc = queue.popleft()
                    for dr, dc in ((-1, 0), (1, 0), (0, -1), (0, 1)):
                        nr, nc = cr + dr, cc + dc
                        if 0 <= nr < rows and 0 <= nc < cols and grid[nr][nc] == 1 and not visited[nr][nc]:
                            visited[nr][nc] = True
                            queue.append((nr, nc))
    return count


def main() -> None:
    grid = [[int(t) for t in line.split()] for line in sys.stdin.read().splitlines() if line]
    print(count_islands(grid))


if __name__ == "__main__":
    main()
