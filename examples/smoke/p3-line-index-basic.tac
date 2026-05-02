let text = @buf-alloc 2 in
let _ = @buf-set text 0 65 in
let _ = @buf-set text 1 66 in
let rows = @i64-alloc 4 in
let count = @line-index text 2 rows in
@add (@mul count 10) (@range-len rows 0)
