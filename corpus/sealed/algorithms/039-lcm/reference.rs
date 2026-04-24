use std::io::{self, Read};

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

fn lcm(a: u64, b: u64) -> u64 {
    if a == 0 || b == 0 {
        0
    } else {
        a / gcd(a, b) * b
    }
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let mut iter = input.split_whitespace();
    let a: u64 = iter.next().unwrap().parse().unwrap();
    let b: u64 = iter.next().unwrap().parse().unwrap();
    println!("{}", lcm(a, b));
}
