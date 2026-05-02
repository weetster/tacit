let src = @i64-alloc 1 in
let _ = @i64-set src 0 12 in
let dst = @i64-alloc 1 in
let _ = @i64-set dst 0 77 in
let _ = @i64-copy dst 0 src 0 0 in
@i64-get dst 0
