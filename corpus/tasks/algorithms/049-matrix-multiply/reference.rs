use std::io::{self, Read};

fn matmul(a: &[Vec<i64>], b: &[Vec<i64>], n: usize, m: usize, p: usize) -> Vec<Vec<i64>> {
    let mut c = vec![vec![0i64; p]; n];
    for i in 0..n {
        for k in 0..m {
            for j in 0..p {
                c[i][j] += a[i][k] * b[k][j];
            }
        }
    }
    c
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let mut tokens = input.split_ascii_whitespace();
    let n: usize = tokens.next().unwrap().parse().unwrap();
    let m: usize = tokens.next().unwrap().parse().unwrap();
    let p: usize = tokens.next().unwrap().parse().unwrap();
    let a: Vec<Vec<i64>> = (0..n)
        .map(|_| (0..m).map(|_| tokens.next().unwrap().parse().unwrap()).collect())
        .collect();
    let b: Vec<Vec<i64>> = (0..m)
        .map(|_| (0..p).map(|_| tokens.next().unwrap().parse().unwrap()).collect())
        .collect();
    for row in matmul(&a, &b, n, m, p) {
        let strs: Vec<String> = row.iter().map(|x| x.to_string()).collect();
        println!("{}", strs.join(" "));
    }
}
