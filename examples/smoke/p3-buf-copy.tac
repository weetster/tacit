let src = @buf-alloc 4 in
let _ = @buf-set src 0 72 in
let _ = @buf-set src 1 105 in
let _ = @buf-set src 2 10 in
let dst = @buf-alloc 4 in
let _ = @buf-copy dst 0 src 0 3 in
let _ = @write 1 dst 3 in
0
