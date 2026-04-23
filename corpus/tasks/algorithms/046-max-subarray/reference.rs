use std::io::{self, Read};

fn max_subarray(xs: &[i64]) -> i64 {
    let mut best = xs[0];
    let mut cur = xs[0];
    for &x in &xs[1..] {
        cur = x.max(cur + x);
        best = best.max(cur);
    }
    best
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let xs: Vec<i64> = input
        .split_whitespace()
        .map(|t| t.parse().unwrap())
        .collect();
    println!("{}", max_subarray(&xs));
}
