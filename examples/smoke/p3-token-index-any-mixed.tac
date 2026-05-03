let text = @buf-alloc 9 in
let _ = @buf-set text 0 32 in
let _ = @buf-set text 1 65 in
let _ = @buf-set text 2 10 in
let _ = @buf-set text 3 10 in
let _ = @buf-set text 4 66 in
let _ = @buf-set text 5 32 in
let _ = @buf-set text 6 32 in
let _ = @buf-set text 7 67 in
let _ = @buf-set text 8 32 in
let delims = @buf-alloc 2 in
let _ = @buf-set delims 0 32 in
let _ = @buf-set delims 1 10 in
let rows = @i64-alloc 18 in
let count = @token-index-any text 0 9 delims 2 rows in
@add (@mul count 50) (@add (@range-start rows 2) (@range-len rows 1))
