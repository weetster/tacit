let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let line_end = @scan-byte input 0 len 10 in
let idx = @buf-alloc 300000 in
let out = @buf-alloc 32 in
rec {
  skip = lambda pos.
    if @ge pos line_end then pos else
      if @eq (@buf-get input pos) 32 then skip (@add pos 1) else pos;
  token_end = lambda pos.
    if @ge pos line_end then line_end else
      if @eq (@buf-get input pos) 32 then pos else token_end (@add pos 1);
  count_tokens = lambda pos. lambda acc.
    let p = skip pos in
    if @ge p line_end then acc else
      let e = token_end p in
      count_tokens (@add e 1) (@add acc 1);
  set_idx = lambda pos. lambda val.
    let off = @mul pos 3 in
    let _ = @buf-set idx off (@mod val 256) in
    let _ = @buf-set idx (@add off 1) (@mod (@div val 256) 256) in
    @buf-set idx (@add off 2) (@div val 65536);
  get_idx = lambda pos.
    let off = @mul pos 3 in
    @add (@buf-get idx off) (@add (@mul (@buf-get idx (@add off 1)) 256) (@mul (@buf-get idx (@add off 2)) 65536));
  init = lambda i. lambda n.
    if @ge i n then 0 else
      let _ = set_idx i i in
      init (@add i 1) n;
  value_at = lambda want. lambda pos. lambda cur.
    let p = skip pos in
    let e = token_end p in
    if @eq cur want then @parse-i64 input p (@sub e p)
    else value_at want (@add e 1) (@add cur 1);
  swap = lambda a. lambda b.
    let av = get_idx a in
    let bv = get_idx b in
    let _ = set_idx a bv in
    set_idx b av;
  adv_i = lambda i. lambda j. lambda pivot.
    if @gt i j then i else
      let v = value_at (get_idx i) 0 0 in
      if @lt v pivot then adv_i (@add i 1) j pivot else i;
  adv_j = lambda i. lambda j. lambda pivot.
    if @gt i j then j else
      let v = value_at (get_idx j) 0 0 in
      if @gt v pivot then adv_j i (@sub j 1) pivot else j;
  partition = lambda i. lambda j. lambda pivot.
    let ni = adv_i i j pivot in
    let nj = adv_j ni j pivot in
    if @le ni nj then
      (let _ = swap ni nj in partition (@add ni 1) (@sub nj 1) pivot)
    else
      @add (@mul (@add ni 1) 100002) (@add nj 1);
  quick = lambda lo. lambda hi.
    if @lt lo hi then
      (let mid = @div (@add lo hi) 2 in
      let pivot = value_at (get_idx mid) 0 0 in
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
  emit_all = lambda i. lambda n. lambda first.
    if @ge i n then first else
      let v = value_at (get_idx i) 0 0 in
      let _ = emit_int v first in
      emit_all (@add i 1) n 0
} in
let n = count_tokens 0 0 in
let _ = init 0 n in
let _ = if @gt n 0 then quick 0 (@sub n 1) else 0 in
let _ = emit_all 0 n 1 in
let _ = @write 1 "\n" 1 in
0
