use std::io::{self, BufRead};

fn parse_line(line: &str) -> Vec<i64> {
    line.split_whitespace()
        .map(|t| t.parse().unwrap())
        .collect()
}

fn merge_sorted(a: &[i64], b: &[i64]) -> Vec<i64> {
    let mut out = Vec::with_capacity(a.len() + b.len());
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i] <= b[j] {
            out.push(a[i]);
            i += 1;
        } else {
            out.push(b[j]);
            j += 1;
        }
    }
    out.extend_from_slice(&a[i..]);
    out.extend_from_slice(&b[j..]);
    out
}

fn main() {
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let line1 = lines.next().unwrap().unwrap();
    let line2 = lines.next().unwrap().unwrap();
    let xs = parse_line(&line1);
    let ys = parse_line(&line2);
    let merged = merge_sorted(&xs, &ys);
    let strs: Vec<String> = merged.iter().map(|x| x.to_string()).collect();
    println!("{}", strs.join(" "));
}
