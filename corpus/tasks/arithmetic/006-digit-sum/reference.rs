use std::io::{self, Read};

fn digit_sum(s: &str) -> u32 {
    s.chars().filter_map(|c| c.to_digit(10)).sum()
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let n = input.trim();
    println!("{}", digit_sum(n));
}
