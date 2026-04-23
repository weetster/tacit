use std::io::{self, BufRead};

fn main() {
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line).unwrap();
    let trimmed = line.strip_suffix('\n').unwrap_or(&line);
    let reversed: String = trimmed.chars().rev().collect();
    println!("{reversed}");
}
