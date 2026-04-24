use std::io::{self, Read};

fn power(base: i64, exp: u32) -> i64 {
    let mut result = 1i64;
    for _ in 0..exp {
        result *= base;
    }
    result
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let mut lines = input.lines();
    let base: i64 = lines.next().unwrap().trim().parse().unwrap();
    let exp: u32 = lines.next().unwrap().trim().parse().unwrap();
    println!("{}", power(base, exp));
}
