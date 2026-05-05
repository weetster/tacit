let buf = @buf-alloc 1 in
let _ = @buf-set buf 0 90 in
let _ = @write-range 1 buf 0 0 in
@buf-get buf 0
