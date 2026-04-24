use std::io::{self, Read};

fn rle_encode(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut count = 1usize;
    for i in 1..chars.len() {
        if chars[i] == chars[i - 1] {
            count += 1;
        } else {
            out.push_str(&count.to_string());
            out.push(chars[i - 1]);
            count = 1;
        }
    }
    out.push_str(&count.to_string());
    out.push(*chars.last().unwrap());
    out
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let s = input.trim_end_matches('\n');
    println!("{}", rle_encode(s));
}
