let buf = @buf-alloc 4 in
let _ = @buf-set buf 0 55 in
@buf-get buf 0
