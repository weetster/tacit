let n = 3 in
let buf = @buf-alloc-dyn n in
let _ = @buf-set buf 0 77 in
@buf-get buf 0
