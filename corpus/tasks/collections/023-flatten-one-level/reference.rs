use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let flat: Vec<String> = input
        .lines()
        .flat_map(|line| line.split_whitespace().map(|t| t.parse::<i64>().unwrap()))
        .map(|x| x.to_string())
        .collect();
    println!("{}", flat.join(" "));
}
