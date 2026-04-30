let ibuf = @buf-alloc 24 in
let n = @read 0 ibuf 24 in
let nl = @scan-byte ibuf 0 n 10 in
let v = @parse-i64 ibuf 0 nl in
let _ = rec {
  emit = lambda n.
    let _ = if @ge n 2 then emit (@div n 2) else 0 in
    let obuf = @buf-alloc 1 in
    let _ = @buf-set obuf 0 (@add (@mod n 2) 48) in
    @write 1 obuf 1
} in emit v in
let _ = @write 1 "\n" 1 in
0
