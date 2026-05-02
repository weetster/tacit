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
  adv_i = lambda i. lambda j. lambda pivot.
    if @gt i j then i else
      if @lt (@i64-get xs i) pivot then adv_i (@add i 1) j pivot else i;
  adv_j = lambda i. lambda j. lambda pivot.
    if @gt i j then j else
      if @gt (@i64-get xs j) pivot then adv_j i (@sub j 1) pivot else j;
  partition = lambda i. lambda j. lambda pivot.
    let ni = adv_i i j pivot in
    let nj = adv_j ni j pivot in
    if @le ni nj then
      (let _ = @i64-swap xs ni nj in partition (@add ni 1) (@sub nj 1) pivot)
    else
      @add (@mul (@add ni 1) 100002) (@add nj 1);
  quick = lambda lo. lambda hi.
    if @lt lo hi then
      (let mid = @div (@add lo hi) 2 in
      let pivot = @i64-get xs mid in
      let packed = partition lo hi pivot in
      let pi = @sub (@div packed 100002) 1 in
      let pj = @sub (@mod packed 100002) 1 in
      let _ = if @lt lo pj then quick lo pj else 0 in
      if @lt pi hi then quick pi hi else 0)
    else 0;
  emit_int = lambda v. lambda first.
    let _ = if first then 0 else @write 1 " " 1 in
    let w = @fmt-i64 out 0 v in
    @write 1 out w;
  emit_all = lambda i. lambda first.
    if @ge i n then first else
      let _ = emit_int (@i64-get xs i) first in
      emit_all (@add i 1) 0
} in
let _ = if @gt n 0 then quick 0 (@sub n 1) else 0 in
let _ = emit_all 0 1 in
let _ = @write 1 "\n" 1 in
0
