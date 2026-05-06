let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let pattern_end = @scan-byte input 0 len 10 in
let text_start = @add pattern_end 1 in
let one = @buf-alloc 1 in
rec {
  line_end = lambda pos.
    @scan-byte input pos (@sub len pos) 10;
  next_line = lambda end.
    if @ge end len then len else @add end 1;
  contains_at = lambda line_start. lambda line_len. lambda pat_len. lambda off.
    if @eq pat_len 0 then @eq 1 1 else
    if @gt (@add off pat_len) line_len then @eq 1 0 else
      if @eq (@buf-eq input (@add line_start off) input 0 pat_len) 1 then @eq 1 1
      else contains_at line_start line_len pat_len (@add off 1);
  emit_span = lambda off. lambda slen. lambda i.
    if @ge i slen then 0 else
      let _ = @buf-set one 0 (@buf-get input (@add off i)) in
      let _ = @write 1 one 1 in
      emit_span off slen (@add i 1);
  loop = lambda pos.
    if @ge pos len then 0 else
      let e = line_end pos in
      let slen = @sub e pos in
      let pat_len = pattern_end in
      let _ = if contains_at pos slen pat_len 0 then
        (let _ = emit_span pos slen 0 in @write 1 "\n" 1)
      else 0 in
      loop (next_line e)
} in loop text_start
