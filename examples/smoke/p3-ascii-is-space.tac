let buf = @buf-alloc 11 in
let _ = @buf-set buf 0 (@add 48 (@ascii-is-space 8)) in
let _ = @buf-set buf 1 (@add 48 (@ascii-is-space 9)) in
let _ = @buf-set buf 2 (@add 48 (@ascii-is-space 10)) in
let _ = @buf-set buf 3 (@add 48 (@ascii-is-space 11)) in
let _ = @buf-set buf 4 (@add 48 (@ascii-is-space 12)) in
let _ = @buf-set buf 5 (@add 48 (@ascii-is-space 13)) in
let _ = @buf-set buf 6 (@add 48 (@ascii-is-space 14)) in
let _ = @buf-set buf 7 (@add 48 (@ascii-is-space 31)) in
let _ = @buf-set buf 8 (@add 48 (@ascii-is-space 32)) in
let _ = @buf-set buf 9 (@add 48 (@ascii-is-space 33)) in
let _ = @buf-set buf 10 (@add 48 (@ascii-is-space 65)) in
let _ = @write 1 buf 11 in
0
