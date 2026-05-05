let buf = @buf-alloc 16 in
let n = @stdin-slurp buf 16 in
let _ = @write-range 1 buf 0 n in
0
