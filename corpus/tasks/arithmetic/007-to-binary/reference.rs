use std::io::{self, Read};

fn to_binary(mut n: u64) -> String {
    if n == 0 {
        return "0".to_string();
    }
    let mut bits = Vec::new();
    while n > 0 {
        bits.push((n & 1) as u8 + b'0');
        n >>= 1;
    }
    bits.reverse();
    String::from_utf8(bits).unwrap()
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let n: u64 = input.trim().parse().unwrap();
    println!("{}", to_binary(n));
}
