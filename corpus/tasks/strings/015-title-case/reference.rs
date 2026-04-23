use std::io::{self, BufRead};

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let mut out = String::with_capacity(word.len());
            out.push(first.to_ascii_uppercase());
            for c in chars {
                out.push(c.to_ascii_lowercase());
            }
            out
        }
    }
}

fn main() {
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line).unwrap();
    let trimmed = line.strip_suffix('\n').unwrap_or(&line);
    let words: Vec<String> = trimmed.split(' ').map(capitalize).collect();
    println!("{}", words.join(" "));
}
