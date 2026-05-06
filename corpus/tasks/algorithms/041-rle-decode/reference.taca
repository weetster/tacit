let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let line_end = @scan-byte input 0 len 10 in
let one = @buf-alloc 1 in
rec {
  digit_end = lambda pos.
    if @ge pos line_end then line_end else
      let b = @buf-get input pos in
      if @lt b 48 then pos else
      if @gt b 57 then pos else digit_end (@add pos 1);
  emit_repeat = lambda byte. lambda count.
    if @le count 0 then 0 else
      let _ = @buf-set one 0 byte in
      let _ = @write 1 one 1 in
      emit_repeat byte (@sub count 1);
  loop = lambda pos.
    if @ge pos line_end then 0 else
      let e = digit_end pos in
      let count = @parse-i64 input pos (@sub e pos) in
      let byte = @buf-get input e in
      let _ = emit_repeat byte count in
      loop (@add e 1)
} in
let _ = loop 0 in
let _ = @write 1 "\n" 1 in
0
