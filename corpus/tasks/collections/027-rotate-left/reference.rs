use std::io::{self, Read};

fn rotate_left(xs: &[i64], k: i64) -> Vec<i64> {
    if xs.is_empty() {
        return Vec::new();
    }
    let n = xs.len() as i64;
    let k = k.rem_euclid(n) as usize;
    let mut out = Vec::with_capacity(xs.len());
    out.extend_from_slice(&xs[k..]);
    out.extend_from_slice(&xs[..k]);
    out
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let mut lines = input.lines();
    let xs: Vec<i64> = lines
        .next()
        .unwrap()
        .split_whitespace()
        .map(|t| t.parse().unwrap())
        .collect();
    let k: i64 = lines.next().unwrap().trim().parse().unwrap();
    let result = rotate_left(&xs, k);
    let strs: Vec<String> = result.iter().map(|x| x.to_string()).collect();
    println!("{}", strs.join(" "));
}
