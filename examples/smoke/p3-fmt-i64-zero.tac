let buf = @buf-alloc 32 in
let n = @fmt-i64 buf 0 0 in
let _ = @write 1 buf n in
0
