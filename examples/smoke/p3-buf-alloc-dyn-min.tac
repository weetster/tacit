let n = 1 in
let buf = @buf-alloc-dyn n in
let _ = @buf-set buf 0 99 in
@buf-get buf 0
