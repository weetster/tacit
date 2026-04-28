let buf = @buf-alloc 4 in
let _ = @buf-set buf 2 88 in
@buf-get buf 2
