let buf = @buf-alloc 6 in
let _ = @buf-set buf 0 72 in
let _ = @buf-set buf 1 101 in
let _ = @buf-set buf 2 108 in
let _ = @buf-set buf 3 108 in
let _ = @buf-set buf 4 111 in
let _ = @buf-set buf 5 10 in
let _ = @buf-rev buf 0 5 in
let _ = @write 1 buf 6 in
0
