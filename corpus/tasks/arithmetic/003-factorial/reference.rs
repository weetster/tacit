use std::io::{self, Read};

fn factorial(n: u64) -> u64 {
    (2..=n).product()
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let n: u64 = input.trim().parse().unwrap();
    println!("{}", factorial(n));
}
