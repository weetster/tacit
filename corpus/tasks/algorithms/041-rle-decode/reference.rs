use std::io::{self, Read};

fn rle_decode(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let j = i;
        let mut k = i;
        while k < chars.len() && chars[k].is_ascii_digit() {
            k += 1;
        }
        let count: usize = chars[j..k].iter().collect::<String>().parse().unwrap();
        let ch = chars[k];
        for _ in 0..count {
            out.push(ch);
        }
        i = k + 1;
    }
    out
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let s = input.trim_end_matches('\n');
    println!("{}", rle_decode(s));
}
