let ibuf = @buf-alloc 8 in
let n = @read 0 ibuf 8 in
let nl = @scan-byte ibuf 0 n 10 in
let v = @parse-i64 ibuf 0 nl in
let result = rec { fact = lambda n. if n then @mul n (fact (@sub n 1)) else 1 } in fact v in
let obuf = @buf-alloc 32 in
let w = @fmt-i64 obuf 0 result in
let _ = @write 1 obuf w in
let _ = @write 1 "\n" 1 in
0
