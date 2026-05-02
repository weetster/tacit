let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let xs = @i64-alloc 100001 in
let out = @buf-alloc 32 in
let n = rec {
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
rec {
  emit_int = lambda v. lambda first.
    let _ = if first then 0 else @write 1 " " 1 in
    let w = @fmt-i64 out 0 v in
    @write 1 out w;
  emit_part = lambda i. lambda want_even. lambda first.
    if @ge i n then first else
      let v = @i64-get xs i in
      let even = @eq (@mod v 2) 0 in
      let take = if want_even then even else if even then @eq 1 0 else @eq 1 1 in
      let next_first = if take then (let _ = emit_int v first in 0) else first in
      emit_part (@add i 1) want_even next_first
} in
let _ = emit_part 0 1 1 in
let _ = @write 1 "\n" 1 in
let _ = emit_part 0 0 1 in
let _ = @write 1 "\n" 1 in
0
