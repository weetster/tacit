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
rec {emit = lambda i. lambda want_even. lambda first.
  if @ge i n then first else
    let v = @i64-get xs i in
    let even = if @eq (@mod v 2) 0 then 1 else 0 in
    if @eq even want_even then
      (let _ = if first then 0 else @write 1 " " 1 in
       let w = @fmt-i64 out 0 v in
       let _ = @write 1 out w in
       emit (@add i 1) want_even 0)
    else emit (@add i 1) want_even first
} in
let _ = emit 0 1 1 in
let _ = @write 1 "\n" 1 in
let _ = emit 0 0 1 in
let _ = @write 1 "\n" 1 in
0
