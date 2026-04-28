let buf = @buf-alloc 6 in
let _ = @buf-set buf 0 104 in
let _ = @buf-set buf 1 101 in
let _ = @buf-set buf 2 108 in
let _ = @buf-set buf 3 108 in
let _ = @buf-set buf 4 111 in
let _ = @buf-set buf 5 10 in
@scan-byte buf 0 6 10
