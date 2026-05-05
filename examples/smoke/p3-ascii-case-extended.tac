let buf = @buf-alloc 10 in
let _ = @buf-set buf 0 (@ascii-tolower 0) in
let _ = @buf-set buf 1 (@ascii-tolower 32) in
let _ = @buf-set buf 2 (@ascii-tolower 127) in
let _ = @buf-set buf 3 (@ascii-tolower 128) in
let _ = @buf-set buf 4 (@ascii-tolower -1) in
let _ = @buf-set buf 5 (@ascii-toupper 0) in
let _ = @buf-set buf 6 (@ascii-toupper 32) in
let _ = @buf-set buf 7 (@ascii-toupper 127) in
let _ = @buf-set buf 8 (@ascii-toupper 255) in
let _ = @buf-set buf 9 (@ascii-toupper -1) in
let _ = @write 1 buf 10 in
0
