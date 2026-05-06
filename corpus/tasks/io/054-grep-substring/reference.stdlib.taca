let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let lines = @i64-alloc (@add (@mul len 2) 2) in
let n = @line-index input len lines in
let one = @buf-alloc 1 in
rec {
  contains_at = lambda line. lambda pat_len. lambda off.
    if @eq pat_len 0 then @eq 1 1 else
    if @gt (@add off pat_len) (@range-len lines line) then @eq 1 0 else
      if @eq (@buf-eq input (@add (@range-start lines line) off) input (@range-start lines 0) pat_len) 1 then @eq 1 1
      else contains_at line pat_len (@add off 1);
  emit_span = lambda off. lambda slen. lambda i.
    if @ge i slen then 0 else
      let _ = @buf-set one 0 (@buf-get input (@add off i)) in
      let _ = @write 1 one 1 in
      emit_span off slen (@add i 1);
  loop = lambda i.
    if @ge i n then 0 else
      let pat_len = @range-len lines 0 in
      let _ = if contains_at i pat_len 0 then
        (let _ = emit_span (@range-start lines i) (@range-len lines i) 0 in @write 1 "\n" 1)
      else 0 in
      loop (@add i 1)
} in loop 1
