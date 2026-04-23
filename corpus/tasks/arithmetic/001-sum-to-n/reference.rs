use std::io::{self, Read};

fn sum_to_n(n: u64) -> u64 {
    n * (n + 1) / 2
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let n: u64 = input.trim().parse().unwrap();
    println!("{}", sum_to_n(n));
}
