let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let one = @buf-alloc 1 in
rec {
  line_end = lambda pos.
    @scan-byte input pos (@sub len pos) 10;
  next_line = lambda end.
    if @ge end len then len else @add end 1;
  span_eq = lambda ao. lambda al. lambda bo. lambda bl.
    if @eq al bl then
      (if @eq al 0 then @eq 1 1 else @eq (@buf-eq input ao input bo al) 1)
    else @eq 1 0;
  seen_before = lambda pos. lambda off. lambda slen.
    if @ge pos off then @eq 1 0 else
      let e = line_end pos in
      let cur_len = @sub e pos in
      if span_eq pos cur_len off slen then @eq 1 1
      else seen_before (next_line e) off slen;
  emit_span = lambda off. lambda slen. lambda i.
    if @ge i slen then 0 else
      let _ = @buf-set one 0 (@buf-get input (@add off i)) in
      let _ = @write 1 one 1 in
      emit_span off slen (@add i 1);
  loop = lambda pos.
    if @ge pos len then 0 else
      let e = line_end pos in
      let slen = @sub e pos in
      let _ = if seen_before 0 pos slen then 0 else
        (let _ = emit_span pos slen 0 in @write 1 "\n" 1) in
      loop (next_line e)
} in loop 0
