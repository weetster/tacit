use std::collections::HashSet;
use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let mut seen: HashSet<&str> = HashSet::new();
    for line in input.lines() {
        if seen.insert(line) {
            println!("{line}");
        }
    }
}
