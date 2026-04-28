let buf = @buf-alloc 3 in
let _ = @buf-set buf 0 52 in
let _ = @buf-set buf 1 50 in
let _ = @buf-set buf 2 0 in
@parse-i64 buf 0 2
