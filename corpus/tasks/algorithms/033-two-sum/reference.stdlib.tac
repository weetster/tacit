let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let table = @i64-alloc 200004 in
let count = @token-index-any input 0 len " \n" 2 table in
let target = @parse-i64 input (@range-start table 0) (@range-len table 0) in
let n = @sub count 1 in
let xs = @i64-alloc 100001 in
let idxs = @i64-alloc 100001 in
let out = @buf-alloc 32 in
let _ = rec {fill = lambda i.
  if @ge i n then 0 else
    let r = @add i 1 in
    let _ = @i64-set xs i (@parse-i64 input (@range-start table r) (@range-len table r)) in
    let _ = @i64-set idxs i i in
    fill (@add i 1)
} in fill 0 in
let _ = @stable-sort-pairs-i64 xs idxs n in
let result = rec {find = lambda lo. lambda hi.
  if @ge lo hi then -1 else
    let s = @add (@i64-get xs lo) (@i64-get xs hi) in
    if @eq s target then
      (let a = @i64-get idxs lo in
       let b = @i64-get idxs hi in
       if @lt a b then @add (@mul a 1000000) b else @add (@mul b 1000000) a)
    else if @lt s target then find (@add lo 1) hi
    else find lo (@sub hi 1)
} in find 0 (@sub n 1) in
let _ = if @lt result 0 then @write 1 "-1\n" 3 else
  let i = @div result 1000000 in
  let j = @mod result 1000000 in
  let w1 = @fmt-i64 out 0 i in
  let _ = @write 1 out w1 in
  let _ = @write 1 " " 1 in
  let w2 = @fmt-i64 out 0 j in
  let _ = @write 1 out w2 in
  @write 1 "\n" 1 in
0
