let xs = @i64-alloc 2 in
let _ = @i64-set xs 0 4 in
let _ = @i64-set xs 1 9 in
let _ = @i64-swap xs 0 1 in
@add (@mul (@i64-get xs 0) 10) (@i64-get xs 1)
