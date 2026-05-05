let buf = @buf-alloc 4 in
let _ = @buf-set buf 0 99 in
let _ = @buf-set buf 1 99 in
let _ = @buf-set buf 2 99 in
let _ = @buf-set buf 3 99 in
let n1 = @utf8-encode buf 0 -1 in
let n2 = @utf8-encode buf 0 55296 in
let n3 = @utf8-encode buf 0 1114112 in
let _ = @write 1 buf 4 in
@add n1 (@add n2 n3)
