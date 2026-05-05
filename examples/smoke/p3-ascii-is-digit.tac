let buf = @buf-alloc 6 in
let _ = @buf-set buf 0 (@add 48 (@ascii-is-digit 47)) in
let _ = @buf-set buf 1 (@add 48 (@ascii-is-digit 48)) in
let _ = @buf-set buf 2 (@add 48 (@ascii-is-digit 53)) in
let _ = @buf-set buf 3 (@add 48 (@ascii-is-digit 57)) in
let _ = @buf-set buf 4 (@add 48 (@ascii-is-digit 58)) in
let _ = @buf-set buf 5 (@add 48 (@ascii-is-digit 65)) in
let _ = @write 1 buf 6 in
0
