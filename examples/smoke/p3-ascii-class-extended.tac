let buf = @buf-alloc 9 in
let _ = @buf-set buf 0 (@add 48 (@ascii-is-alpha 0)) in
let _ = @buf-set buf 1 (@add 48 (@ascii-is-alpha 127)) in
let _ = @buf-set buf 2 (@add 48 (@ascii-is-alpha 128)) in
let _ = @buf-set buf 3 (@add 48 (@ascii-is-alpha 255)) in
let _ = @buf-set buf 4 (@add 48 (@ascii-is-digit 200)) in
let _ = @buf-set buf 5 (@add 48 (@ascii-is-digit 0)) in
let _ = @buf-set buf 6 (@add 48 (@ascii-is-space 200)) in
let _ = @buf-set buf 7 (@add 48 (@ascii-is-space 0)) in
let _ = @buf-set buf 8 (@add 48 (@ascii-is-alpha -1)) in
let _ = @write 1 buf 9 in
0
