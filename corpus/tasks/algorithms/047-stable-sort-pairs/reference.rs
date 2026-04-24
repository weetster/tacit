use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let mut pairs: Vec<(i64, &str)> = input
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| {
            let (k, v) = l.split_once(' ').unwrap();
            (k.parse().unwrap(), v)
        })
        .collect();
    pairs.sort_by_key(|&(k, _)| k);
    for (k, v) in pairs {
        println!("{k} {v}");
    }
}
