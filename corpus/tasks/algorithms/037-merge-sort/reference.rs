use std::io::{self, Read};

fn merge(a: Vec<i64>, b: Vec<i64>) -> Vec<i64> {
    let mut result = Vec::with_capacity(a.len() + b.len());
    let mut i = 0;
    let mut j = 0;
    while i < a.len() && j < b.len() {
        if a[i] <= b[j] {
            result.push(a[i]);
            i += 1;
        } else {
            result.push(b[j]);
            j += 1;
        }
    }
    result.extend_from_slice(&a[i..]);
    result.extend_from_slice(&b[j..]);
    result
}

fn merge_sort(xs: Vec<i64>) -> Vec<i64> {
    if xs.len() <= 1 {
        return xs;
    }
    let mid = xs.len() / 2;
    let left = merge_sort(xs[..mid].to_vec());
    let right = merge_sort(xs[mid..].to_vec());
    merge(left, right)
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let xs: Vec<i64> = input
        .split_whitespace()
        .map(|t| t.parse().unwrap())
        .collect();
    let sorted = merge_sort(xs);
    let strs: Vec<String> = sorted.iter().map(|x| x.to_string()).collect();
    println!("{}", strs.join(" "));
}
