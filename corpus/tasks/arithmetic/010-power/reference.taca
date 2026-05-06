let ibuf = @buf-alloc 32 in
let n = @read 0 ibuf 32 in
let nl1 = @scan-byte ibuf 0 n 10 in
let b = @parse-i64 ibuf 0 nl1 in
let nl1p1 = @add nl1 1 in
let nl2 = @scan-byte ibuf nl1p1 (@sub n nl1p1) 10 in
let e = @parse-i64 ibuf nl1p1 (@sub nl2 nl1p1) in
let result = rec {
  pow = lambda base. lambda exp.
    if exp then @mul base (pow base (@sub exp 1)) else 1
} in pow b e in
let obuf = @buf-alloc 32 in
let w = @fmt-i64 obuf 0 result in
let _ = @write 1 obuf w in
let _ = @write 1 "\n" 1 in
0
