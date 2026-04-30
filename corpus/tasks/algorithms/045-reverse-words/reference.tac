let input = @buf-alloc 10000 in
let len = @read 0 input 10000 in
let line_end = @scan-byte input 0 len 10 in
let one = @buf-alloc 1 in
rec {
  prev_space = lambda pos.
    if @lt pos 0 then -1 else
      if @eq (@buf-get input pos) 32 then pos else prev_space (@sub pos 1);
  emit_span = lambda off. lambda slen. lambda i.
    if @ge i slen then 0 else
      let _ = @buf-set one 0 (@buf-get input (@add off i)) in
      let _ = @write 1 one 1 in
      emit_span off slen (@add i 1);
  loop = lambda end. lambda first.
    if @le end 0 then 0 else
      let sp = prev_space (@sub end 1) in
      let start = @add sp 1 in
      let slen = @sub end start in
      let _ = if first then 0 else @write 1 " " 1 in
      let _ = emit_span start slen 0 in
      if @lt sp 0 then 0 else loop sp 0
} in
let _ = loop line_end 1 in
let _ = @write 1 "\n" 1 in
0
