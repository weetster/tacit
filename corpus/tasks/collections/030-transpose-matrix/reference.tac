let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let first_end = @scan-byte input 0 len 10 in
let out = @buf-alloc 32 in
let cols = rec {
  skip = lambda pos. lambda end.
    if @ge pos end then pos else
      if @eq (@buf-get input pos) 32 then skip (@add pos 1) end else pos;
  token_end = lambda pos. lambda end.
    if @ge pos end then end else
      if @eq (@buf-get input pos) 32 then pos else token_end (@add pos 1) end;
  count_cols = lambda pos. lambda acc.
    let p = skip pos first_end in
    if @ge p first_end then acc else
      let e = token_end p first_end in
      count_cols (@add e 1) (@add acc 1)
} in count_cols 0 0 in
let _ = rec {
  skip = lambda pos. lambda end.
    if @ge pos end then pos else
      if @eq (@buf-get input pos) 32 then skip (@add pos 1) end else pos;
  token_end = lambda pos. lambda end.
    if @ge pos end then end else
      if @eq (@buf-get input pos) 32 then pos else token_end (@add pos 1) end;
  nth = lambda pos. lambda end. lambda target. lambda idx.
    let p = skip pos end in
    let e = token_end p end in
    if @eq idx target then @add (@mul p 1000001) (@sub e p)
    else nth (@add e 1) end target (@add idx 1);
  emit_int = lambda v. lambda first.
    let _ = if first then 0 else @write 1 " " 1 in
    let w = @fmt-i64 out 0 v in
    @write 1 out w;
  emit_col = lambda col. lambda row_pos. lambda first.
    if @ge row_pos len then first else
      let row_end = @scan-byte input row_pos (@sub len row_pos) 10 in
      let span = nth row_pos row_end col 0 in
      let off = @div span 1000001 in
      let slen = @mod span 1000001 in
      let v = @parse-i64 input off slen in
      let _ = emit_int v first in
      let next_row = if @ge row_end len then len else @add row_end 1 in
      emit_col col next_row 0;
  emit_cols = lambda col.
    if @ge col cols then 0 else
      let _ = emit_col col 0 1 in
      let _ = @write 1 "\n" 1 in
      emit_cols (@add col 1)
} in emit_cols 0 in
0
