let input = @buf-alloc 1048576 in
let n = @stdin-slurp input 1048576 in
let count = rec {
  loop = lambda i. lambda in_word. lambda c.
    if @ge i n then c else
      let ws = @ascii-is-space (@buf-get input i) in
      if @eq ws 1 then loop (@add i 1) 0 c
      else if @eq in_word 1 then loop (@add i 1) 1 c
      else loop (@add i 1) 1 (@add c 1)
} in loop 0 0 0 in
let out = @buf-alloc 32 in
let w = @fmt-i64 out 0 count in
let _ = @write 1 out w in
let _ = @write 1 "\n" 1 in
0
