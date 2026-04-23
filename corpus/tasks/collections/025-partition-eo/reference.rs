use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let xs: Vec<i64> = input
        .split_whitespace()
        .map(|t| t.parse().unwrap())
        .collect();
    let evens: Vec<String> = xs
        .iter()
        .filter(|&&x| x % 2 == 0)
        .map(|x| x.to_string())
        .collect();
    let odds: Vec<String> = xs
        .iter()
        .filter(|&&x| x % 2 != 0)
        .map(|x| x.to_string())
        .collect();
    println!("{}", evens.join(" "));
    println!("{}", odds.join(" "));
}
