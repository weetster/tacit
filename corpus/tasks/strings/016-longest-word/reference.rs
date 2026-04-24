use std::io::{self, Read};

fn longest_word(line: &str) -> &str {
    let mut best = "";
    for word in line.split(' ') {
        if word.len() > best.len() {
            best = word;
        }
    }
    best
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let line = input.trim_end_matches('\n');
    println!("{}", longest_word(line));
}
