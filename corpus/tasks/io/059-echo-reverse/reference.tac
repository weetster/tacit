let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let one = @buf-alloc 1 in
rec {
  prev_newline = lambda pos.
    if @lt pos 0 then -1 else
      if @eq (@buf-get input pos) 10 then pos else prev_newline (@sub pos 1);
  emit_span = lambda off. lambda slen. lambda i.
    if @ge i slen then 0 else
      let _ = @buf-set one 0 (@buf-get input (@add off i)) in
      let _ = @write 1 one 1 in
      emit_span off slen (@add i 1);
  loop = lambda end.
    if @lt end 0 then 0 else
      let sp = prev_newline (@sub end 1) in
      let start = @add sp 1 in
      let slen = @sub end start in
      let _ = emit_span start slen 0 in
      let _ = @write 1 "\n" 1 in
      if @lt sp 0 then 0 else loop sp
} in
let end = if @eq len 0 then -1 else
  (if @eq (@buf-get input (@sub len 1)) 10 then @sub len 1 else len) in
loop end
