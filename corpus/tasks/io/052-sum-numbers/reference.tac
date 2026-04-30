let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let out = @buf-alloc 32 in
let result = rec {
  line_end = lambda pos.
    @scan-byte input pos (@sub len pos) 10;
  next_line = lambda end.
    if @ge end len then len else @add end 1;
  loop = lambda pos. lambda acc.
    if @ge pos len then acc else
      let e = line_end pos in
      let next_acc = if @eq e pos then acc else @add acc (@parse-i64 input pos (@sub e pos)) in
      loop (next_line e) next_acc
} in loop 0 0 in
let w = @fmt-i64 out 0 result in
let _ = @write 1 out w in
let _ = @write 1 "\n" 1 in
0
