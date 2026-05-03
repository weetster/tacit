let xs = @i64-alloc 4 in
let _ = @i64-set xs 0 9 in
let _ = @i64-set xs 1 -2 in
let _ = @i64-set xs 2 9 in
let _ = @i64-set xs 3 0 in
let _ = @sort-i64 xs 4 in
let ok0 = @eq (@i64-get xs 0) -2 in
let ok1 = @eq (@i64-get xs 1) 0 in
let ok2 = @eq (@i64-get xs 2) 9 in
let ok3 = @eq (@i64-get xs 3) 9 in
@add (@add ok0 ok1) (@add ok2 ok3)
