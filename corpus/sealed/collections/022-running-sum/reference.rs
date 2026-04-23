use std::io::{self, Read};

fn running_sum(xs: &[i64]) -> Vec<i64> {
    let mut acc: i64 = 0;
    xs.iter()
        .map(|&x| {
            acc += x;
            acc
        })
        .collect()
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let xs: Vec<i64> = input
        .split_whitespace()
        .map(|t| t.parse().unwrap())
        .collect();
    let result = running_sum(&xs);
    let strs: Vec<String> = result.iter().map(|x| x.to_string()).collect();
    println!("{}", strs.join(" "));
}
