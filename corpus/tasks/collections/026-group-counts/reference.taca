let input = @buf-alloc 100000 in
let len = @read 0 input 100000 in
let one = @buf-alloc 1 in
let out = @buf-alloc 32 in
let base = 100001 in
let base2 = 10000200001 in
rec {
  skip = lambda pos.
    if @ge pos len then pos else
      let b = @buf-get input pos in
      if @eq b 32 then skip (@add pos 1) else
      if @eq b 10 then skip (@add pos 1) else pos;
  word_end = lambda pos.
    if @ge pos len then len else
      let b = @buf-get input pos in
      if @eq b 32 then pos else
      if @eq b 10 then pos else word_end (@add pos 1);
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
    if @eq al bl then @eq (@buf-eq input ao input bo al) 1 else @eq 1 0;
  pack_best = lambda have. lambda off. lambda slen.
    @add (@mul have base2) (@add (@mul off base) slen);
  find_best = lambda pos. lambda have_best. lambda best_off. lambda best_len. lambda has_prev. lambda prev_off. lambda prev_len.
    let p = skip pos in
    if @ge p len then pack_best have_best best_off best_len else
      let e = word_end p in
      let slen = @sub e p in
      let after_prev = if has_prev then span_lt prev_off prev_len p slen else @eq 1 1 in
      let better = if after_prev then
        (if have_best then span_lt p slen best_off best_len else @eq 1 1)
      else @eq 1 0 in
      let next_have = if better then 1 else have_best in
      let next_off = if better then p else best_off in
      let next_len = if better then slen else best_len in
      find_best (@add e 1) next_have next_off next_len has_prev prev_off prev_len;
  count_eq = lambda pos. lambda off. lambda slen. lambda acc.
    let p = skip pos in
    if @ge p len then acc else
      let e = word_end p in
      let cur_len = @sub e p in
      let next_acc = if span_eq p cur_len off slen then @add acc 1 else acc in
      count_eq (@add e 1) off slen next_acc;
  emit_word = lambda off. lambda slen. lambda i.
    if @ge i slen then 0 else
      let _ = @buf-set one 0 (@buf-get input (@add off i)) in
      let _ = @write 1 one 1 in
      emit_word off slen (@add i 1);
  emit_pair = lambda off. lambda slen. lambda count.
    let _ = emit_word off slen 0 in
    let _ = @write 1 ":" 1 in
    let w = @fmt-i64 out 0 count in
    let _ = @write 1 out w in
    @write 1 "\n" 1;
  loop = lambda has_prev. lambda prev_off. lambda prev_len.
    let packed = find_best 0 0 0 0 has_prev prev_off prev_len in
    let have = @div packed base2 in
    if @eq have 0 then 0 else
      let rest = @mod packed base2 in
      let off = @div rest base in
      let slen = @mod rest base in
      let count = count_eq 0 off slen 0 in
      let _ = emit_pair off slen count in
      loop 1 off slen
} in loop 0 0 0
