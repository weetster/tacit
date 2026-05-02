let text = @buf-alloc 4 in
let _ = @buf-set text 0 10 in
let _ = @buf-set text 1 65 in
let _ = @buf-set text 2 10 in
let _ = @buf-set text 3 10 in
let rows = @i64-alloc 8 in
let count = @line-index text 4 rows in
let row1_start = @range-start rows 1 in
let row1_len = @range-len rows 1 in
let row2_start = @range-start rows 2 in
@add (@mul count 40) (@add row1_start (@add row1_len row2_start))
