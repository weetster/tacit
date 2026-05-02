let xs = @i64-alloc 3 in
let _ = @i64-set xs 0 7 in
let _ = @i64-set xs 1 0 in
let _ = @i64-set xs 2 -2 in
let ok0 = @eq (@i64-get xs 0) 7 in
let ok1 = @eq (@i64-get xs 1) 0 in
let ok2 = @eq (@i64-get xs 2) -2 in
@add (@add ok0 ok1) ok2
