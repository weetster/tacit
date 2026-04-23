use std::io::{self, Read};

fn unique_in_order(xs: &[i64]) -> Vec<i64> {
    let mut out: Vec<i64> = Vec::new();
    for &x in xs {
        if out.last() != Some(&x) {
            out.push(x);
        }
    }
    out
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let xs: Vec<i64> = input
        .split_whitespace()
        .map(|t| t.parse().unwrap())
        .collect();
    let result = unique_in_order(&xs);
    let strs: Vec<String> = result.iter().map(|x| x.to_string()).collect();
    println!("{}", strs.join(" "));
}
