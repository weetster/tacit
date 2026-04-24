use std::collections::HashSet;
use std::io::{self, Read};

fn is_valid_row(digits: &[u8]) -> bool {
    let mut seen = HashSet::new();
    for &d in digits {
        if d != 0 && !seen.insert(d) {
            return false;
        }
    }
    true
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let digits: Vec<u8> = input
        .split_whitespace()
        .map(|t| t.parse().unwrap())
        .collect();
    println!("{}", if is_valid_row(&digits) { "valid" } else { "invalid" });
}
