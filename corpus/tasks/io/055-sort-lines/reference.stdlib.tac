let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let spans = @i64-alloc (@add (@mul len 2) 2) in
let one = @buf-alloc 1 in
let n = @line-index input len spans in
rec {
  start = lambda i. @range-start spans i;
  slen = lambda i. @range-len spans i;
  span_lt_loop = lambda ao. lambda al. lambda bo. lambda bl. lambda j.
    if @ge j al then @lt al bl else
    if @ge j bl then @eq 1 0 else
      let ab = @buf-get input (@add ao j) in
      let bb = @buf-get input (@add bo j) in
      if @lt ab bb then @eq 1 1 else
      if @gt ab bb then @eq 1 0 else
        span_lt_loop ao al bo bl (@add j 1);
  span_lt = lambda a. lambda b.
    span_lt_loop (start a) (slen a) (start b) (slen b) 0;
  swap_pair = lambda a. lambda b.
    let ao = @mul a 2 in
    let bo = @mul b 2 in
    let _ = @i64-swap spans ao bo in
    @i64-swap spans (@add ao 1) (@add bo 1);
  inner = lambda j. lambda limit.
    if @ge j limit then 0 else
      let next = @add j 1 in
      let _ = if span_lt next j then swap_pair j next else 0 in
      inner next limit;
  outer = lambda i.
    if @ge i n then 0 else
      let _ = inner 0 (@sub (@sub n 1) i) in
      outer (@add i 1);
  emit_span = lambda off. lambda count. lambda j.
    if @ge j count then 0 else
      let _ = @buf-set one 0 (@buf-get input (@add off j)) in
      let _ = @write 1 one 1 in
      emit_span off count (@add j 1);
  emit_all = lambda i.
    if @ge i n then 0 else
      let _ = emit_span (start i) (slen i) 0 in
      let _ = @write 1 "\n" 1 in
      emit_all (@add i 1)
} in
let _ = outer 0 in
emit_all 0
