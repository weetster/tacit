let src = @i64-alloc 3 in
let _ = @i64-set src 0 11 in
let _ = @i64-set src 1 22 in
let _ = @i64-set src 2 33 in
let dst = @i64-alloc 3 in
let _ = @i64-set dst 0 1 in
let _ = @i64-copy dst 1 src 0 2 in
@add (@i64-get dst 1) (@i64-get dst 2)
