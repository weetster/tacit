let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
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
  emit_int = lambda v. lambda first.
    let _ = if first then 0 else @write 1 " " 1 in
    let w = @fmt-i64 out 0 v in
    @write 1 out w;
  loop = lambda pos. lambda have. lambda prev. lambda first.
    let p = skip pos in
    if @ge p len then first else
      let e = token_end p in
      let v = @parse-i64 input p (@sub e p) in
      let take = if have then @ne v prev else @eq 1 1 in
      let next_first = if take then (let _ = emit_int v first in 0) else first in
      loop (@add e 1) 1 v next_first
} in loop 0 0 0 1 in
let _ = @write 1 "\n" 1 in
0
