use std::io::{self, Read};

fn bubble_sort(xs: &mut [i64]) {
    let n = xs.len();
    for i in 0..n {
        for j in 0..n.saturating_sub(1 + i) {
            if xs[j] > xs[j + 1] {
                xs.swap(j, j + 1);
            }
        }
    }
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let mut xs: Vec<i64> = input
        .split_whitespace()
        .map(|t| t.parse().unwrap())
        .collect();
    bubble_sort(&mut xs);
    let strs: Vec<String> = xs.iter().map(|x| x.to_string()).collect();
    println!("{}", strs.join(" "));
}
