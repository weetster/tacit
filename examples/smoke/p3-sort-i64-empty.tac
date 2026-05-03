let xs = @i64-alloc 1 in
let _ = @i64-set xs 0 42 in
let _ = @sort-i64 xs 0 in
@i64-get xs 0
