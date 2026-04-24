use std::io::{self, Read};

fn common_prefix<'a>(lines: &'a [&str]) -> &'a str {
    if lines.is_empty() {
        return "";
    }
    let mut prefix = lines[0];
    for line in &lines[1..] {
        let len = prefix
            .chars()
            .zip(line.chars())
            .take_while(|(a, b)| a == b)
            .count();
        prefix = &prefix[..prefix
            .char_indices()
            .nth(len)
            .map(|(i, _)| i)
            .unwrap_or(prefix.len())];
        if prefix.is_empty() {
            break;
        }
    }
    prefix
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let lines: Vec<&str> = input.lines().collect();
    println!("{}", common_prefix(&lines));
}
