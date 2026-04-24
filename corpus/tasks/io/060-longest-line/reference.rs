use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let lines: Vec<&str> = input.lines().collect();
    let mut best = lines[0];
    for line in &lines[1..] {
        if line.chars().count() > best.chars().count() {
            best = line;
        }
    }
    println!("{}", best);
}
