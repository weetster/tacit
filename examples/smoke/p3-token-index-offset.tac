let text = @buf-alloc 10 in
let _ = @buf-set text 0 120 in
let _ = @buf-set text 1 120 in
let _ = @buf-set text 2 32 in
let _ = @buf-set text 3 65 in
let _ = @buf-set text 4 32 in
let _ = @buf-set text 5 32 in
let _ = @buf-set text 6 66 in
let _ = @buf-set text 7 32 in
let _ = @buf-set text 8 121 in
let _ = @buf-set text 9 121 in
let rows = @i64-alloc 8 in
let count = @token-index text 2 6 288 rows in
let first_start = @range-start rows 0 in
let second_start = @range-start rows 1 in
let second_len = @range-len rows 1 in
@add (@mul count 50) (@add (@mul first_start 3) (@add second_start second_len))
