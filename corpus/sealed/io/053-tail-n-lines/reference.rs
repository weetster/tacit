use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let mut lines = input.lines();
    let n: usize = lines.next().unwrap_or("0").trim().parse().unwrap();
    let rest: Vec<&str> = lines.collect();
    let start = rest.len().saturating_sub(n);
    for line in &rest[start..] {
        println!("{}", line);
    }
}
