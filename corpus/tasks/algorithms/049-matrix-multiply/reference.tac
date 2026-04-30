let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let out = @buf-alloc 32 in
rec {
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
  value_at = lambda want. lambda pos. lambda idx.
    let p = skip pos in
    let e = token_end p in
    if @eq idx want then @parse-i64 input p (@sub e p)
    else value_at want (@add e 1) (@add idx 1);
  dot = lambda i. lambda j. lambda k. lambda acc. lambda n. lambda m. lambda pcols.
    if @ge k m then acc else
      let a_idx = @add 3 (@add (@mul i m) k) in
      let b_idx = @add (@add 3 (@mul n m)) (@add (@mul k pcols) j) in
      let av = value_at a_idx 0 0 in
      let bv = value_at b_idx 0 0 in
      dot i j (@add k 1) (@add acc (@mul av bv)) n m pcols;
  emit_int = lambda v. lambda first.
    let _ = if first then 0 else @write 1 " " 1 in
    let w = @fmt-i64 out 0 v in
    @write 1 out w;
  emit_row = lambda i. lambda j. lambda n. lambda m. lambda pcols.
    if @ge j pcols then @write 1 "\n" 1 else
      let v = dot i j 0 0 n m pcols in
      let _ = emit_int v (@eq j 0) in
      emit_row i (@add j 1) n m pcols;
  emit_rows = lambda i. lambda n. lambda m. lambda pcols.
    if @ge i n then 0 else
      let _ = emit_row i 0 n m pcols in
      emit_rows (@add i 1) n m pcols
} in
let n = value_at 0 0 0 in
let m = value_at 1 0 0 in
let pcols = value_at 2 0 0 in
emit_rows 0 n m pcols
