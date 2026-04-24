use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let rows: Vec<Vec<&str>> = input
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.split_whitespace().collect())
        .collect();
    let ncols = rows[0].len();
    for col in 0..ncols {
        let out: Vec<&str> = rows.iter().map(|row| row[col]).collect();
        println!("{}", out.join(" "));
    }
}
