let input = @buf-alloc 64 in
let len = @read 0 input 64 in
let line_end = @scan-byte input 0 len 10 in
rec {
  skip = lambda pos.
    if @ge pos line_end then pos else
      if @eq (@buf-get input pos) 32 then skip (@add pos 1) else pos;
  token_end = lambda pos.
    if @ge pos line_end then line_end else
      if @eq (@buf-get input pos) 32 then pos else token_end (@add pos 1);
  has_dup = lambda pos. lambda value.
    let p = skip pos in
    if @ge p line_end then @eq 1 0 else
      let e = token_end p in
      let cur = @parse-i64 input p (@sub e p) in
      if @eq cur value then @eq 1 1 else has_dup (@add e 1) value;
  check = lambda pos.
    let p = skip pos in
    if @ge p line_end then @eq 1 1 else
      let e = token_end p in
      let cur = @parse-i64 input p (@sub e p) in
      if @eq cur 0 then check (@add e 1) else
      if has_dup (@add e 1) cur then @eq 1 0 else check (@add e 1)
} in
let ok = check 0 in
let _ = if ok then @write 1 "valid\n" 6 else @write 1 "invalid\n" 8 in
0
