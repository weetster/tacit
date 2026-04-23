use std::io::{self, BufRead};

fn is_palindrome(s: &str) -> bool {
    s.chars().eq(s.chars().rev())
}

fn main() {
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line).unwrap();
    let line = line.strip_suffix('\n').unwrap_or(&line);
    println!("{}", if is_palindrome(line) { "yes" } else { "no" });
}
