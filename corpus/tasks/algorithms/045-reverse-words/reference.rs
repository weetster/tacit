use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let line = input.trim_end_matches('\n');
    let words: Vec<&str> = line.split(' ').collect();
    let reversed: Vec<&str> = words.into_iter().rev().collect();
    println!("{}", reversed.join(" "));
}
