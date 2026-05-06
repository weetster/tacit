let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let spans = @i64-alloc (@add (@mul len 2) 2) in
let n = @line-index input len spans in
let scratch = @buf-alloc 1000002 in
rec {
  span_eq = lambda a. lambda b.
    let al = @range-len spans a in
    let bl = @range-len spans b in
    if @eq al bl then
      (if @eq al 0 then @eq 1 1 else @eq (@buf-eq input (@range-start spans a) input (@range-start spans b) al) 1)
    else @eq 1 0;
  seen = lambda j. lambda i.
    if @ge j i then @eq 1 0 else
      if span_eq j i then @eq 1 1 else seen (@add j 1) i;
  emit = lambda i.
    if @ge i n then 0 else
      let _ = if seen 0 i then 0 else
        (let off = @range-start spans i in
         let l = @range-len spans i in
         let _ = @buf-copy scratch 0 input off l in
         let _ = @buf-set scratch l 10 in
         @write 1 scratch (@add l 1)) in
      emit (@add i 1)
} in emit 0
