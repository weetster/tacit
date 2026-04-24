use std::collections::BTreeMap;
use std::io::{self, BufRead};

fn main() {
    let stdin = io::stdin();
    let line = stdin.lock().lines().next().unwrap().unwrap();
    let mut counts: BTreeMap<char, usize> = BTreeMap::new();
    for c in line.chars() {
        *counts.entry(c).or_insert(0) += 1;
    }
    for (ch, count) in &counts {
        println!("{}:{}", ch, count);
    }
}
