let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let one = @buf-alloc 1 in
rec {
  line_end = lambda pos.
    @scan-byte input pos (@sub len pos) 10;
  key_end = lambda pos. lambda end.
    if @ge pos end then end else
      if @eq (@buf-get input pos) 32 then pos else key_end (@add pos 1) end;
  key_at = lambda pos.
    let e = key_end pos (line_end pos) in
    @parse-i64 input pos (@sub e pos);
  next_line = lambda end.
    if @ge end len then len else @add end 1;
  find_min = lambda pos. lambda have. lambda best_off. lambda has_prev. lambda prev_key.
    if @ge pos len then
      (if have then @add best_off 1 else 0)
    else
      let end = line_end pos in
      let key = key_at pos in
      let after_prev = if has_prev then @gt key prev_key else @eq 1 1 in
      let better = if after_prev then
        (if have then @lt key (key_at best_off) else @eq 1 1)
      else @eq 1 0 in
      let next_have = if better then 1 else have in
      let next_best = if better then pos else best_off in
      find_min (next_line end) next_have next_best has_prev prev_key;
  emit_span = lambda off. lambda slen. lambda i.
    if @ge i slen then 0 else
      let _ = @buf-set one 0 (@buf-get input (@add off i)) in
      let _ = @write 1 one 1 in
      emit_span off slen (@add i 1);
  emit_matching = lambda pos. lambda target.
    if @ge pos len then 0 else
      let end = line_end pos in
      let key = key_at pos in
      let _ = if @eq key target then
        (let _ = emit_span pos (@sub end pos) 0 in @write 1 "\n" 1)
      else 0 in
      emit_matching (next_line end) target;
  loop = lambda has_prev. lambda prev_key.
    let packed = find_min 0 0 0 has_prev prev_key in
    if @eq packed 0 then 0 else
      let off = @sub packed 1 in
      let key = key_at off in
      let _ = emit_matching 0 key in
      loop 1 key
} in loop 0 0
