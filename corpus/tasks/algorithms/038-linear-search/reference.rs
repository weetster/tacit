use std::io::{self, Read};

fn linear_search(xs: &[i64], target: i64) -> i64 {
    for (i, &x) in xs.iter().enumerate() {
        if x == target {
            return i as i64;
        }
    }
    -1
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let mut lines = input.lines();
    let first = lines.next().unwrap_or("");
    let xs: Vec<i64> = if first.trim().is_empty() {
        vec![]
    } else {
        first.split_whitespace().map(|t| t.parse().unwrap()).collect()
    };
    let target: i64 = lines.next().unwrap().trim().parse().unwrap();
    println!("{}", linear_search(&xs, target));
}
