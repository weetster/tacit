use std::collections::HashMap;
use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let words: Vec<&str> = input.split_whitespace().collect();
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for word in &words {
        *counts.entry(word).or_insert(0) += 1;
    }
    let mut pairs: Vec<(&&str, &usize)> = counts.iter().collect();
    pairs.sort_by_key(|(w, _)| **w);
    for (word, count) in pairs {
        println!("{}:{}", word, count);
    }
}
