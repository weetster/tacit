let xs = @i64-alloc 4 in
let _ = @i64-set xs 0 1 in
let _ = @i64-set xs 1 2 in
let _ = @i64-set xs 2 3 in
let _ = @i64-set xs 3 4 in
let _ = @i64-copy xs 1 xs 0 3 in
@add (@mul (@i64-get xs 2) 10) (@i64-get xs 3)
