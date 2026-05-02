let n = @add 2 1 in
let xs = @i64-alloc n in
let _ = @i64-set xs 2 44 in
@i64-get xs 2
