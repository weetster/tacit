let buf = @buf-alloc 2 in
let _ = @buf-set buf 0 65 in
let _ = @buf-set buf 1 66 in
let _ = @buf-rev buf 0 0 in
@buf-get buf 0
