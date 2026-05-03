let xs = @i64-alloc 5 in
let _ = @i64-set xs 0 -3 in
let _ = @i64-set xs 1 0 in
let _ = @i64-set xs 2 4 in
let _ = @i64-set xs 3 4 in
let _ = @i64-set xs 4 9 in
let hit = @lower-bound-i64 xs 5 4 in
let insert = @lower-bound-i64 xs 5 5 in
@add (@mul hit 10) insert
