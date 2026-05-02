let xs = @i64-alloc 1 in
let _ = @i64-set xs 0 33 in
let _ = @i64-swap xs 0 0 in
@i64-get xs 0
