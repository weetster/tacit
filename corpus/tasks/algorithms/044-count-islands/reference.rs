use std::collections::VecDeque;
use std::io::{self, Read};

fn count_islands(grid: &mut Vec<Vec<u8>>) -> usize {
    let rows = grid.len() as isize;
    let cols = grid[0].len() as isize;
    let mut count = 0;
    for r in 0..rows {
        for c in 0..cols {
            if grid[r as usize][c as usize] == 1 {
                count += 1;
                grid[r as usize][c as usize] = 0;
                let mut queue = VecDeque::new();
                queue.push_back((r, c));
                while let Some((cr, cc)) = queue.pop_front() {
                    for (dr, dc) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                        let nr = cr + dr;
                        let nc = cc + dc;
                        if nr >= 0 && nr < rows && nc >= 0 && nc < cols && grid[nr as usize][nc as usize] == 1 {
                            grid[nr as usize][nc as usize] = 0;
                            queue.push_back((nr, nc));
                        }
                    }
                }
            }
        }
    }
    count
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let mut grid: Vec<Vec<u8>> = input
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.split_whitespace().map(|t| t.parse().unwrap()).collect())
        .collect();
    println!("{}", count_islands(&mut grid));
}
