let buf = @buf-alloc 10 in
let n1 = @utf8-encode buf 0 65 in
let n2 = @utf8-encode buf 1 233 in
let n3 = @utf8-encode buf 3 20013 in
let n4 = @utf8-encode buf 6 128512 in
let _ = @write 1 buf 10 in
@add n1 (@add n2 (@add n3 n4))
