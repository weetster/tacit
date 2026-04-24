use std::io::{self, BufRead};

fn shift_char(c: char, k: i64) -> char {
    if c.is_ascii_lowercase() {
        let shifted = ((c as i64 - 'a' as i64 + k).rem_euclid(26)) as u8 + b'a';
        shifted as char
    } else if c.is_ascii_uppercase() {
        let shifted = ((c as i64 - 'A' as i64 + k).rem_euclid(26)) as u8 + b'A';
        shifted as char
    } else {
        c
    }
}

fn main() {
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let k: i64 = lines.next().unwrap().unwrap().trim().parse().unwrap();
    let text = lines.next().map(|l| l.unwrap()).unwrap_or_default();
    let result: String = text.chars().map(|c| shift_char(c, k)).collect();
    println!("{}", result);
}
