let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let table = @i64-alloc 360010 in
let count = @token-index-any input 0 len " \n" 2 table in
let xs = @i64-alloc 180005 in
let out = @buf-alloc 32 in
let _ = rec {fill = lambda i.
  if @ge i count then 0 else
    let _ = @i64-set xs i (@parse-i64 input (@range-start table i) (@range-len table i)) in
    fill (@add i 1)
} in fill 0 in
let n = @i64-get xs 0 in
let m = @i64-get xs 1 in
let pcols = @i64-get xs 2 in
let bbase = @add 3 (@mul n m) in
rec {
  dot = lambda i. lambda j. lambda k. lambda acc.
    if @ge k m then acc else
      let a = @i64-get xs (@add 3 (@add (@mul i m) k)) in
      let b = @i64-get xs (@add bbase (@add (@mul k pcols) j)) in
      dot i j (@add k 1) (@add acc (@mul a b));
  emit_row = lambda i. lambda j.
    if @ge j pcols then @write 1 "\n" 1 else
      let _ = if @eq j 0 then 0 else @write 1 " " 1 in
      let w = @fmt-i64 out 0 (dot i j 0 0) in
      let _ = @write 1 out w in
      emit_row i (@add j 1);
  emit_rows = lambda i.
    if @ge i n then 0 else
      let _ = emit_row i 0 in
      emit_rows (@add i 1)
} in emit_rows 0
