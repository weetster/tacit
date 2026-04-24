use std::io::{self, Read};

fn longest_run(xs: &[i64]) -> usize {
    if xs.is_empty() {
        return 0;
    }
    let mut best = 1;
    let mut cur = 1;
    for i in 1..xs.len() {
        if xs[i] == xs[i - 1] {
            cur += 1;
            if cur > best {
                best = cur;
            }
        } else {
            cur = 1;
        }
    }
    best
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let xs: Vec<i64> = input.split_whitespace().map(|t| t.parse().unwrap()).collect();
    println!("{}", longest_run(&xs));
}
