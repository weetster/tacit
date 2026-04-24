use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let mut lines = input.lines();
    let first = lines.next().unwrap_or("");
    let xs: Vec<i64> = if first.trim().is_empty() {
        vec![]
    } else {
        first.split_whitespace().map(|t| t.parse().unwrap()).collect()
    };
    let k: usize = lines.next().unwrap().trim().parse().unwrap();
    for chunk in xs.chunks(k) {
        let out: Vec<String> = chunk.iter().map(|x| x.to_string()).collect();
        println!("{}", out.join(" "));
    }
}
