let buf = @buf-alloc 8 in
let _ = @buf-set buf 0 (@ascii-tolower 64) in
let _ = @buf-set buf 1 (@ascii-tolower 65) in
let _ = @buf-set buf 2 (@ascii-tolower 90) in
let _ = @buf-set buf 3 (@ascii-tolower 91) in
let _ = @buf-set buf 4 (@ascii-tolower 96) in
let _ = @buf-set buf 5 (@ascii-tolower 97) in
let _ = @buf-set buf 6 (@ascii-tolower 122) in
let _ = @buf-set buf 7 (@ascii-tolower 123) in
let _ = @write 1 buf 8 in
0
