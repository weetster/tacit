use std::io::{self, BufRead};

fn main() {
    let total: i64 = io::stdin()
        .lock()
        .lines()
        .map(|l| l.unwrap())
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.trim().parse::<i64>().unwrap())
        .sum();
    println!("{total}");
}
