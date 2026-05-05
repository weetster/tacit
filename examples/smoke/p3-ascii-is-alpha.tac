let buf = @buf-alloc 8 in
let _ = @buf-set buf 0 (@add 48 (@ascii-is-alpha 64)) in
let _ = @buf-set buf 1 (@add 48 (@ascii-is-alpha 65)) in
let _ = @buf-set buf 2 (@add 48 (@ascii-is-alpha 90)) in
let _ = @buf-set buf 3 (@add 48 (@ascii-is-alpha 91)) in
let _ = @buf-set buf 4 (@add 48 (@ascii-is-alpha 96)) in
let _ = @buf-set buf 5 (@add 48 (@ascii-is-alpha 97)) in
let _ = @buf-set buf 6 (@add 48 (@ascii-is-alpha 122)) in
let _ = @buf-set buf 7 (@add 48 (@ascii-is-alpha 123)) in
let _ = @write 1 buf 8 in
0
