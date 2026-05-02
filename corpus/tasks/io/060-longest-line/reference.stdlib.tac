let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let lines = @i64-alloc (@add (@mul len 2) 2) in
let n = @line-index input len lines in
let one = @buf-alloc 1 in
rec {
  emit_span = lambda off. lambda slen. lambda i.
    if @ge i slen then 0 else
      let _ = @buf-set one 0 (@buf-get input (@add off i)) in
      let _ = @write 1 one 1 in
      emit_span off slen (@add i 1);
  best_line = lambda i. lambda best.
    if @ge i n then best else
      let ilen = @range-len lines i in
      let blen = @range-len lines best in
      if @gt ilen blen then best_line (@add i 1) i
      else best_line (@add i 1) best
} in
let best = best_line 1 0 in
let _ = emit_span (@range-start lines best) (@range-len lines best) 0 in
let _ = @write 1 "\n" 1 in
0
