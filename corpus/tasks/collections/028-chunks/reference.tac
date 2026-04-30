let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let line_end = @scan-byte input 0 len 10 in
let k_start = @add line_end 1 in
let k_end = @scan-byte input k_start (@sub len k_start) 10 in
let k = @parse-i64 input k_start (@sub k_end k_start) in
let out = @buf-alloc 32 in
let _ = rec {
  skip = lambda pos.
    if @ge pos line_end then pos else
      if @eq (@buf-get input pos) 32 then skip (@add pos 1) else pos;
  token_end = lambda pos.
    if @ge pos line_end then line_end else
      if @eq (@buf-get input pos) 32 then pos else token_end (@add pos 1);
  emit_int = lambda v. lambda first.
    let _ = if first then 0 else @write 1 " " 1 in
    let w = @fmt-i64 out 0 v in
    @write 1 out w;
  loop = lambda pos. lambda col. lambda seen.
    let p = skip pos in
    if @ge p line_end then
      (if seen then
        (if @eq col 0 then 0 else @write 1 "\n" 1)
      else 0)
    else
      let e = token_end p in
      let v = @parse-i64 input p (@sub e p) in
      let _ = emit_int v (@eq col 0) in
      let next_col = @add col 1 in
      if @eq next_col k then
        (let _ = @write 1 "\n" 1 in loop (@add e 1) 0 1)
      else
        loop (@add e 1) next_col 1
} in loop 0 0 0 in
0
