let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let one = @buf-alloc 1 in
let base = 1000001 in
rec {
  line_end = lambda pos.
    @scan-byte input pos (@sub len pos) 10;
  next_line = lambda end.
    if @ge end len then len else @add end 1;
  pack = lambda off. lambda slen.
    @add (@mul off base) slen;
  loop = lambda pos. lambda have. lambda best_off. lambda best_len.
    if @ge pos len then pack best_off best_len else
      let e = line_end pos in
      let slen = @sub e pos in
      let better = if have then @gt slen best_len else @eq 1 1 in
      let next_off = if better then pos else best_off in
      let next_len = if better then slen else best_len in
      loop (next_line e) 1 next_off next_len;
  emit_span = lambda off. lambda slen. lambda i.
    if @ge i slen then 0 else
      let _ = @buf-set one 0 (@buf-get input (@add off i)) in
      let _ = @write 1 one 1 in
      emit_span off slen (@add i 1)
} in
let packed = loop 0 0 0 0 in
let off = @div packed base in
let slen = @mod packed base in
let _ = emit_span off slen 0 in
let _ = @write 1 "\n" 1 in
0
