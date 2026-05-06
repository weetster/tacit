let ibuf = @buf-alloc 24 in
let n = @read 0 ibuf 24 in
let nl = @scan-byte ibuf 0 n 10 in
let v = @parse-i64 ibuf 0 nl in
let result = rec { digit_sum = lambda n. if n then @add (@mod n 10) (digit_sum (@div n 10)) else 0 } in digit_sum v in
let obuf = @buf-alloc 8 in
let w = @fmt-i64 obuf 0 result in
let _ = @write 1 obuf w in
let _ = @write 1 "\n" 1 in
0
