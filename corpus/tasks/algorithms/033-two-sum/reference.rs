use std::collections::HashMap;
use std::io::{self, BufRead};

fn two_sum(target: i64, nums: &[i64]) -> Option<(usize, usize)> {
    let mut seen: HashMap<i64, usize> = HashMap::new();
    for (i, &n) in nums.iter().enumerate() {
        if let Some(&j) = seen.get(&(target - n)) {
            return Some((j, i));
        }
        seen.insert(n, i);
    }
    None
}

fn main() {
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let target: i64 = lines.next().unwrap().unwrap().trim().parse().unwrap();
    let nums: Vec<i64> = lines
        .next()
        .unwrap()
        .unwrap()
        .split_whitespace()
        .map(|t| t.parse().unwrap())
        .collect();
    match two_sum(target, &nums) {
        Some((i, j)) => println!("{i} {j}"),
        None => println!("-1"),
    }
}
