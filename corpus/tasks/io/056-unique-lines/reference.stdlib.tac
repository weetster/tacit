let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let spans = @i64-alloc (@add (@mul len 2) 2) in
let one = @buf-alloc 1 in
let n = @line-index input len spans in
rec {
  start = lambda i. @range-start spans i;
  slen = lambda i. @range-len spans i;
  span_eq = lambda a. lambda b.
    let al = slen a in
    let bl = slen b in
    if @eq al bl then
      (if @eq al 0 then @eq 1 1 else @eq (@buf-eq input (start a) input (start b) al) 1)
    else @eq 1 0;
  seen = lambda j. lambda i.
    if @ge j i then @eq 1 0 else
      if span_eq j i then @eq 1 1 else seen (@add j 1) i;
  emit_span = lambda off. lambda count. lambda j.
    if @ge j count then 0 else
      let _ = @buf-set one 0 (@buf-get input (@add off j)) in
      let _ = @write 1 one 1 in
      emit_span off count (@add j 1);
  emit_unique = lambda i.
    if @ge i n then 0 else
      let _ = if seen 0 i then 0 else
        (let _ = emit_span (start i) (slen i) 0 in @write 1 "\n" 1) in
      emit_unique (@add i 1)
} in emit_unique 0
