use std::io::{self, BufRead};

fn parse_line(line: &str) -> Vec<i64> {
    line.split_whitespace()
        .map(|t| t.parse().unwrap())
        .collect()
}

fn main() {
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let line1 = lines.next().unwrap().unwrap();
    let line2 = lines.next().unwrap().unwrap();
    let xs = parse_line(&line1);
    let ys = parse_line(&line2);
    for (x, y) in xs.iter().zip(ys.iter()) {
        println!("{} {}", x, y);
    }
}
