use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let count = if input.is_empty() {
        0
    } else {
        let nl = input.matches('\n').count();
        if input.ends_with('\n') { nl } else { nl + 1 }
    };
    println!("{count}");
}
