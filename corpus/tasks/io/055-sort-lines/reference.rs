use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let mut lines: Vec<&str> = input.lines().collect();
    lines.sort();
    for line in lines {
        println!("{line}");
    }
}
