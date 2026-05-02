let rows = @i64-alloc 4 in
let _ = @i64-set rows 0 7 in
let _ = @i64-set rows 1 4 in
let _ = @i64-set rows 2 9 in
let _ = @i64-set rows 3 2 in
@add (@mul (@range-start rows 1) 10) (@range-len rows 0)
