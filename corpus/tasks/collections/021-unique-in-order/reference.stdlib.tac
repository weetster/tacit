let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let table = @i64-alloc 200002 in
let n = @token-index-any input 0 len " \n" 2 table in
let xs = @i64-alloc 100001 in
let out = @buf-alloc 32 in
let _ = rec {fill = lambda i.
  if @ge i n then 0 else
    let _ = @i64-set xs i (@parse-i64 input (@range-start table i) (@range-len table i)) in
    fill (@add i 1)
} in fill 0 in
let _ = rec {emit = lambda i. lambda have. lambda prev.
  if @ge i n then 0 else
    let v = @i64-get xs i in
    let skip = if have then @eq v prev else @eq 1 0 in
    if skip then emit (@add i 1) have prev
    else
      (let _ = if have then @write 1 " " 1 else 0 in
       let w = @fmt-i64 out 0 v in
       let _ = @write 1 out w in
       emit (@add i 1) 1 v)
} in emit 0 0 0 in
let _ = @write 1 "\n" 1 in
0
