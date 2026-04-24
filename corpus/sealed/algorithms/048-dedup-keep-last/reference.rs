use std::collections::HashMap;
use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let nums: Vec<i64> = input.split_whitespace().map(|t| t.parse().unwrap()).collect();

    let mut last_pos: HashMap<i64, usize> = HashMap::new();
    for (i, &n) in nums.iter().enumerate() {
        last_pos.insert(n, i);
    }

    let result: Vec<String> = nums
        .iter()
        .enumerate()
        .filter(|(i, n)| last_pos[n] == *i)
        .map(|(_, n)| n.to_string())
        .collect();

    println!("{}", result.join(" "));
}
