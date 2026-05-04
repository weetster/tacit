let input = @buf-alloc 32768 in
let len = @read 0 input 32768 in
let table = @i64-alloc 2002 in
let n = @token-index-any input 0 len " \n" 2 table in
let xs = @i64-alloc 1001 in
let out = @buf-alloc 32 in
let _ = rec {fill = lambda i.
  if @ge i n then 0 else
    let _ = @i64-set xs i (@parse-i64 input (@range-start table i) (@range-len table i)) in
    fill (@add i 1)
} in fill 0 in
let _ = rec {
  inner = lambda j. lambda limit.
    if @ge j limit then 0 else
      let next = @add j 1 in
      let _ = if @gt (@i64-get xs j) (@i64-get xs next) then @i64-swap xs j next else 0 in
      inner next limit;
  outer = lambda i.
    if @ge i n then 0 else
      let _ = inner 0 (@sub (@sub n 1) i) in
      outer (@add i 1)
} in outer 0 in
let _ = rec {emit = lambda i.
  if @ge i n then 0 else
    let _ = if @eq i 0 then 0 else @write 1 " " 1 in
    let w = @fmt-i64 out 0 (@i64-get xs i) in
    let _ = @write 1 out w in
    emit (@add i 1)
} in emit 0 in
let _ = @write 1 "\n" 1 in
0
