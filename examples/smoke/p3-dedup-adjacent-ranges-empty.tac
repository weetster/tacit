let text = @buf-alloc 1 in
let rows = @i64-alloc 2 in
let out = @i64-alloc 2 in
@dedup-adjacent-ranges text rows 0 out
