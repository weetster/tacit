let text = @buf-alloc 5 in
let _ = @buf-set text 0 97 in
let _ = @buf-set text 1 98 in
let _ = @buf-set text 2 97 in
let _ = @buf-set text 3 98 in
let _ = @buf-set text 4 99 in
let rows = @i64-alloc 6 in
let _ = @i64-set rows 0 0 in
let _ = @i64-set rows 1 2 in
let _ = @i64-set rows 2 2 in
let _ = @i64-set rows 3 2 in
let _ = @i64-set rows 4 4 in
let _ = @i64-set rows 5 1 in
let groups = @dedup-adjacent-ranges text rows 3 rows in
let ok_count = @eq groups 2 in
let ok0 = @add (@eq (@range-start rows 0) 0) (@eq (@range-len rows 0) 2) in
let ok1 = @add (@eq (@range-start rows 1) 4) (@eq (@range-len rows 1) 1) in
@add ok_count (@add ok0 ok1)
