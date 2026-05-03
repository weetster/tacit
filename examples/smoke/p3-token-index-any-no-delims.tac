let text = @buf-alloc 3 in
let _ = @buf-set text 0 65 in
let _ = @buf-set text 1 66 in
let _ = @buf-set text 2 67 in
let delims = @buf-alloc 1 in
let rows = @i64-alloc 6 in
let count = @token-index-any text 0 3 delims 0 rows in
@add (@mul count 10) (@range-len rows 0)
