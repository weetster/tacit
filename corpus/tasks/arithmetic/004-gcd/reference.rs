use std::io::{self, Read};

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let mut it = input.split_whitespace();
    let a: u64 = it.next().unwrap().parse().unwrap();
    let b: u64 = it.next().unwrap().parse().unwrap();
    println!("{}", gcd(a, b));
}
