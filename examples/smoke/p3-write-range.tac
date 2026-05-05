let buf = @buf-alloc 6 in
let _ = @buf-set buf 0 72 in
let _ = @buf-set buf 1 101 in
let _ = @buf-set buf 2 108 in
let _ = @buf-set buf 3 108 in
let _ = @buf-set buf 4 111 in
let _ = @buf-set buf 5 10 in
let _ = @write-range 1 buf 1 4 in
0
