//! Dump `<hash-hex>  <filename>` for every *.canonical fixture.
//! Run from repo root; pass the fixture directory as argv[1].

use std::env;
use std::fs;
use std::path::PathBuf;

use tacit_canon::{hash_node, parse};

fn main() {
    let dir: PathBuf = env::args()
        .nth(1)
        .map(PathBuf::from)
        .expect("usage: dump-hashes <fixture-dir>");
    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("canonical"))
        .collect();
    paths.sort();
    for p in paths {
        let data = fs::read(&p).unwrap();
        let node = match parse(&data) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("parse failed for {}: {}", p.display(), e);
                std::process::exit(1);
            }
        };
        let h = hash_node(&node);
        let hex: String = h.iter().map(|b| format!("{:02x}", b)).collect();
        println!("{}  {}", hex, p.file_name().unwrap().to_string_lossy());
    }
}
