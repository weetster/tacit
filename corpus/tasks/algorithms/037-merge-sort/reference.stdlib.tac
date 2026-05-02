let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let xs = @i64-alloc 100001 in
let aux = @i64-alloc 100001 in
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
  merge = lambda i. lambda mid. lambda j. lambda hi. lambda k.
    if @ge i mid then @i64-copy aux k xs j (@sub hi j) else
    if @ge j hi then @i64-copy aux k xs i (@sub mid i) else
      let lv = @i64-get xs i in
      let rv = @i64-get xs j in
      if @le lv rv then
        (let _ = @i64-set aux k lv in merge (@add i 1) mid j hi (@add k 1))
      else
        (let _ = @i64-set aux k rv in merge i mid (@add j 1) hi (@add k 1));
  sort = lambda lo. lambda hi.
    if @le (@sub hi lo) 1 then 0 else
      let mid = @div (@add lo hi) 2 in
      let _ = sort lo mid in
      let _ = sort mid hi in
      let _ = merge lo mid mid hi lo in
      @i64-copy xs lo aux lo (@sub hi lo);
  emit_int = lambda v. lambda first.
    let _ = if first then 0 else @write 1 " " 1 in
    let w = @fmt-i64 out 0 v in
    @write 1 out w;
  emit_all = lambda i. lambda first.
    if @ge i n then first else
      let _ = emit_int (@i64-get xs i) first in
      emit_all (@add i 1) 0
} in
let _ = sort 0 n in
let _ = emit_all 0 1 in
let _ = @write 1 "\n" 1 in
0
