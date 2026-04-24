use std::io::{self, Read};

fn quicksort(xs: Vec<i64>) -> Vec<i64> {
    if xs.len() <= 1 {
        return xs;
    }
    let pivot = xs[xs.len() / 2];
    let mut left = Vec::new();
    let mut mid = Vec::new();
    let mut right = Vec::new();
    for x in xs {
        if x < pivot {
            left.push(x);
        } else if x == pivot {
            mid.push(x);
        } else {
            right.push(x);
        }
    }
    let mut result = quicksort(left);
    result.extend(mid);
    result.extend(quicksort(right));
    result
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let xs: Vec<i64> = input
        .split_whitespace()
        .map(|t| t.parse().unwrap())
        .collect();
    let sorted = quicksort(xs);
    let strs: Vec<String> = sorted.iter().map(|x| x.to_string()).collect();
    println!("{}", strs.join(" "));
}
