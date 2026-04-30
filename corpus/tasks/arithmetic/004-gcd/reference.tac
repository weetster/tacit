let ibuf = @buf-alloc 48 in
let n = @read 0 ibuf 48 in
let sp = @scan-byte ibuf 0 n 32 in
let a0 = @parse-i64 ibuf 0 sp in
let sp1 = @add sp 1 in
let nl = @scan-byte ibuf sp1 (@sub n sp1) 10 in
let b0 = @parse-i64 ibuf sp1 (@sub nl sp1) in
let result = rec {
  gcd = lambda a. lambda b.
    if b then gcd b (@mod a b) else a
} in gcd a0 b0 in
let obuf = @buf-alloc 32 in
let w = @fmt-i64 obuf 0 result in
let _ = @write 1 obuf w in
let _ = @write 1 "\n" 1 in
0
