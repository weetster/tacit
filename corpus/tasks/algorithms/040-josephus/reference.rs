use std::io::{self, Read};

fn josephus(n: u64, k: u64) -> u64 {
    let mut pos: u64 = 0;
    for i in 2..=n {
        pos = (pos + k) % i;
    }
    pos
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let mut lines = input.lines();
    let n: u64 = lines.next().unwrap().trim().parse().unwrap();
    let k: u64 = lines.next().unwrap().trim().parse().unwrap();
    println!("{}", josephus(n, k));
}
