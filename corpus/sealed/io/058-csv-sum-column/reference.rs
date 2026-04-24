use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let mut lines = input.lines();
    let col: usize = lines.next().unwrap_or("0").trim().parse().unwrap();
    let total: i64 = lines
        .filter(|l| !l.is_empty())
        .map(|l| l.split(',').nth(col).unwrap_or("0").trim().parse::<i64>().unwrap())
        .sum();
    println!("{}", total);
}
