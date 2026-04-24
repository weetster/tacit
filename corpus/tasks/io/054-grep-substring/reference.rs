use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let mut lines = input.lines();
    let pattern = lines.next().unwrap_or("");
    for line in lines {
        if line.contains(pattern) {
            println!("{}", line);
        }
    }
}
