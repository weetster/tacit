let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let one = @buf-alloc 1 in
let base = 1000001 in
let base2 = 1000002000001 in
rec {
  line_end = lambda pos.
    @scan-byte input pos (@sub len pos) 10;
  next_line = lambda end.
    if @ge end len then len else @add end 1;
  span_lt = lambda ao. lambda al. lambda bo. lambda bl.
    span_lt_loop ao al bo bl 0;
  span_lt_loop = lambda ao. lambda al. lambda bo. lambda bl. lambda i.
    if @ge i al then @lt al bl else
    if @ge i bl then @eq 1 0 else
      let ab = @buf-get input (@add ao i) in
      let bb = @buf-get input (@add bo i) in
      if @lt ab bb then @eq 1 1 else
      if @gt ab bb then @eq 1 0 else
        span_lt_loop ao al bo bl (@add i 1);
  span_eq = lambda ao. lambda al. lambda bo. lambda bl.
    if @eq al bl then
      (if @eq al 0 then @eq 1 1 else @eq (@buf-eq input ao input bo al) 1)
    else @eq 1 0;
  pack = lambda have. lambda off. lambda slen.
    @add (@mul have base2) (@add (@mul off base) slen);
  find_best = lambda pos. lambda have. lambda best_off. lambda best_len. lambda has_prev. lambda prev_off. lambda prev_len.
    if @ge pos len then pack have best_off best_len else
      let e = line_end pos in
      let slen = @sub e pos in
      let after_prev = if has_prev then span_lt prev_off prev_len pos slen else @eq 1 1 in
      let better = if after_prev then
        (if have then span_lt pos slen best_off best_len else @eq 1 1)
      else @eq 1 0 in
      let next_have = if better then 1 else have in
      let next_off = if better then pos else best_off in
      let next_len = if better then slen else best_len in
      find_best (next_line e) next_have next_off next_len has_prev prev_off prev_len;
  emit_span = lambda off. lambda slen. lambda i.
    if @ge i slen then 0 else
      let _ = @buf-set one 0 (@buf-get input (@add off i)) in
      let _ = @write 1 one 1 in
      emit_span off slen (@add i 1);
  emit_matching = lambda pos. lambda off. lambda slen.
    if @ge pos len then 0 else
      let e = line_end pos in
      let cur_len = @sub e pos in
      let _ = if span_eq pos cur_len off slen then
        (let _ = emit_span pos cur_len 0 in @write 1 "\n" 1)
      else 0 in
      emit_matching (next_line e) off slen;
  loop = lambda has_prev. lambda prev_off. lambda prev_len.
    let packed = find_best 0 0 0 0 has_prev prev_off prev_len in
    let have = @div packed base2 in
    if @eq have 0 then 0 else
      let rest = @mod packed base2 in
      let off = @div rest base in
      let slen = @mod rest base in
      let _ = emit_matching 0 off slen in
      loop 1 off slen
} in loop 0 0 0
