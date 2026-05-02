let input = @buf-alloc 100000 in
let len = @read 0 input 100000 in
let spans = @i64-alloc 200002 in
let one = @buf-alloc 1 in
let out = @buf-alloc 32 in
let n = rec {
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
  load = lambda pos. lambda i.
    let p = skip pos in
    if @ge p len then i else
      let e = word_end p in
      let off = @mul i 2 in
      let _ = @i64-set spans off p in
      let _ = @i64-set spans (@add off 1) (@sub e p) in
      load (@add e 1) (@add i 1)
} in load 0 0 in
rec {
  start = lambda i. @i64-get spans (@mul i 2);
  slen = lambda i. @i64-get spans (@add (@mul i 2) 1);
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
  span_eq = lambda a. lambda b.
    let al = slen a in
    let bl = slen b in
    if @eq al bl then @eq (@buf-eq input (start a) input (start b) al) 1 else @eq 1 0;
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
  emit_word = lambda off. lambda count. lambda j.
    if @ge j count then 0 else
      let _ = @buf-set one 0 (@buf-get input (@add off j)) in
      let _ = @write 1 one 1 in
      emit_word off count (@add j 1);
  same_end = lambda j. lambda base.
    if @ge j n then j else
      if span_eq base j then same_end (@add j 1) base else j;
  emit_groups = lambda i.
    if @ge i n then 0 else
      let j = same_end (@add i 1) i in
      let _ = emit_word (start i) (slen i) 0 in
      let _ = @write 1 ":" 1 in
      let w = @fmt-i64 out 0 (@sub j i) in
      let _ = @write 1 out w in
      let _ = @write 1 "\n" 1 in
      emit_groups j
} in
let _ = outer 0 in
emit_groups 0
