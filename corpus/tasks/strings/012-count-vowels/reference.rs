use std::io::{self, Read};

fn count_vowels(text: &str) -> usize {
    text.chars().filter(|c| "aeiouAEIOU".contains(*c)).count()
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    println!("{}", count_vowels(&input));
}
