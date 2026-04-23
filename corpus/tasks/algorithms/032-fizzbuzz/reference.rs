use std::io::{self, Read};

fn fizzbuzz(i: u32) -> String {
    match (i % 3, i % 5) {
        (0, 0) => "FizzBuzz".to_string(),
        (0, _) => "Fizz".to_string(),
        (_, 0) => "Buzz".to_string(),
        _ => i.to_string(),
    }
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let n: u32 = input.trim().parse().unwrap();
    for i in 1..=n {
        println!("{}", fizzbuzz(i));
    }
}
