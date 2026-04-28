let buf = @buf-alloc 1024 in
let n = @read 0 buf 1024 in
let _ = @write 1 buf n in
0
