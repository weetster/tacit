let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let line_end = @scan-byte input 0 len 10 in
let idx = @buf-alloc 300000 in
let aux = @buf-alloc 300000 in
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
  set_aux = lambda pos. lambda val.
    let off = @mul pos 3 in
    let _ = @buf-set aux off (@mod val 256) in
    let _ = @buf-set aux (@add off 1) (@mod (@div val 256) 256) in
    @buf-set aux (@add off 2) (@div val 65536);
  get_aux = lambda pos.
    let off = @mul pos 3 in
    @add (@buf-get aux off) (@add (@mul (@buf-get aux (@add off 1)) 256) (@mul (@buf-get aux (@add off 2)) 65536));
  init = lambda i. lambda n.
    if @ge i n then 0 else
      let _ = set_idx i i in
      init (@add i 1) n;
  value_at = lambda want. lambda pos. lambda cur.
    let p = skip pos in
    let e = token_end p in
    if @eq cur want then @parse-i64 input p (@sub e p)
    else value_at want (@add e 1) (@add cur 1);
  copy_left = lambda i. lambda mid. lambda k.
    if @ge i mid then 0 else
      let _ = set_aux k (get_idx i) in
      copy_left (@add i 1) mid (@add k 1);
  copy_right = lambda j. lambda hi. lambda k.
    if @ge j hi then 0 else
      let _ = set_aux k (get_idx j) in
      copy_right (@add j 1) hi (@add k 1);
  merge = lambda i. lambda mid. lambda j. lambda hi. lambda k.
    if @ge i mid then copy_right j hi k else
    if @ge j hi then copy_left i mid k else
      let li = get_idx i in
      let rj = get_idx j in
      let lv = value_at li 0 0 in
      let rv = value_at rj 0 0 in
      if @le lv rv then
        (let _ = set_aux k li in merge (@add i 1) mid j hi (@add k 1))
      else
        (let _ = set_aux k rj in merge i mid (@add j 1) hi (@add k 1));
  copy_back = lambda pos. lambda hi.
    if @ge pos hi then 0 else
      let _ = set_idx pos (get_aux pos) in
      copy_back (@add pos 1) hi;
  merge_sort = lambda lo. lambda hi.
    if @le (@sub hi lo) 1 then 0 else
      let mid = @div (@add lo hi) 2 in
      let _ = merge_sort lo mid in
      let _ = merge_sort mid hi in
      let _ = merge lo mid mid hi lo in
      copy_back lo hi;
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
let _ = merge_sort 0 n in
let _ = emit_all 0 n 1 in
let _ = @write 1 "\n" 1 in
0
