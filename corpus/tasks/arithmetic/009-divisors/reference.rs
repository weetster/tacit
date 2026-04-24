use std::io::{self, Read};

fn proper_divisors(n: u64) -> Vec<u64> {
    if n == 1 {
        return vec![];
    }
    let mut divs = Vec::new();
    let mut i = 1u64;
    while i * i <= n {
        if n % i == 0 {
            divs.push(i);
            let j = n / i;
            if j != i && j != n {
                divs.push(j);
            }
        }
        i += 1;
    }
    divs.sort_unstable();
    divs
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let n: u64 = input.trim().parse().unwrap();
    let divs = proper_divisors(n);
    let out: Vec<String> = divs.iter().map(|d| d.to_string()).collect();
    println!("{}", out.join(" "));
}
