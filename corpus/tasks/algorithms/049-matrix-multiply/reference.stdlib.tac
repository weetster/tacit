let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let xs = @i64-alloc 200000 in
let out = @buf-alloc 32 in
let _ = rec {
  skip = lambda pos.
    if @ge pos len then pos else
      let b = @buf-get input pos in
      if @eq b 32 then skip (@add pos 1) else
      if @eq b 10 then skip (@add pos 1) else pos;
  token_end = lambda pos.
    if @ge pos len then len else
      let b = @buf-get input pos in
      if @eq b 32 then pos else
      if @eq b 10 then pos else token_end (@add pos 1);
  load = lambda pos. lambda i.
    let p = skip pos in
    if @ge p len then i else
      let e = token_end p in
      let _ = @i64-set xs i (@parse-i64 input p (@sub e p)) in
      load (@add e 1) (@add i 1)
} in load 0 0 in
let n = @i64-get xs 0 in
let m = @i64-get xs 1 in
let pcols = @i64-get xs 2 in
rec {
  dot = lambda i. lambda j. lambda k. lambda acc.
    if @ge k m then acc else
      let a_idx = @add 3 (@add (@mul i m) k) in
      let b_idx = @add (@add 3 (@mul n m)) (@add (@mul k pcols) j) in
      dot i j (@add k 1) (@add acc (@mul (@i64-get xs a_idx) (@i64-get xs b_idx)));
  emit_int = lambda v. lambda first.
    let _ = if first then 0 else @write 1 " " 1 in
    let w = @fmt-i64 out 0 v in
    @write 1 out w;
  emit_row = lambda i. lambda j.
    if @ge j pcols then @write 1 "\n" 1 else
      let _ = emit_int (dot i j 0 0) (@eq j 0) in
      emit_row i (@add j 1);
  emit_rows = lambda i.
    if @ge i n then 0 else
      let _ = emit_row i 0 in
      emit_rows (@add i 1)
} in emit_rows 0
