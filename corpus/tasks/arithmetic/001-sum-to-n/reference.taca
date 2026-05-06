let ibuf = @buf-alloc 32 in
let n = @read 0 ibuf 32 in
let nl = @scan-byte ibuf 0 n 10 in
let v = @parse-i64 ibuf 0 nl in
let result = @div (@mul v (@add v 1)) 2 in
let obuf = @buf-alloc 32 in
let w = @fmt-i64 obuf 0 result in
let _ = @write 1 obuf w in
let _ = @write 1 "\n" 1 in
0
