let buf = @buf-alloc 8 in
let _ = @buf-set buf 0 (@ascii-toupper 64) in
let _ = @buf-set buf 1 (@ascii-toupper 65) in
let _ = @buf-set buf 2 (@ascii-toupper 90) in
let _ = @buf-set buf 3 (@ascii-toupper 91) in
let _ = @buf-set buf 4 (@ascii-toupper 96) in
let _ = @buf-set buf 5 (@ascii-toupper 97) in
let _ = @buf-set buf 6 (@ascii-toupper 122) in
let _ = @buf-set buf 7 (@ascii-toupper 123) in
let _ = @write 1 buf 8 in
0
