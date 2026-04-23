use std::io::{self, Read};

fn is_anagram(a: &str, b: &str) -> bool {
    let mut ac: Vec<char> = a.chars().collect();
    let mut bc: Vec<char> = b.chars().collect();
    ac.sort_unstable();
    bc.sort_unstable();
    ac == bc
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let mut lines = input.lines();
    let a = lines.next().unwrap_or("");
    let b = lines.next().unwrap_or("");
    println!("{}", if is_anagram(a, b) { "true" } else { "false" });
}
