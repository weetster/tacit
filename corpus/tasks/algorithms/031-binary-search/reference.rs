use std::io::{self, Read};

fn binary_search(xs: &[i64], target: i64) -> i64 {
    match xs.binary_search(&target) {
        Ok(i) => i as i64,
        Err(_) => -1,
    }
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let mut lines = input.lines();
    let xs: Vec<i64> = lines
        .next()
        .unwrap()
        .split_whitespace()
        .map(|t| t.parse().unwrap())
        .collect();
    let target: i64 = lines.next().unwrap().trim().parse().unwrap();
    println!("{}", binary_search(&xs, target));
}
