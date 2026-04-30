let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let out = @buf-alloc 32 in
let result = rec {
  is_ws = lambda byte.
    if @eq byte 32 then @eq 1 1 else
    if @eq byte 9 then @eq 1 1 else
    if @eq byte 10 then @eq 1 1 else
    if @eq byte 13 then @eq 1 1 else
    if @eq byte 12 then @eq 1 1 else
    if @eq byte 11 then @eq 1 1 else @eq 1 0;
  loop = lambda pos. lambda in_word. lambda count.
    if @ge pos len then count else
      let byte = @buf-get input pos in
      if is_ws byte then loop (@add pos 1) 0 count else
      if in_word then loop (@add pos 1) 1 count else
        loop (@add pos 1) 1 (@add count 1)
} in loop 0 0 0 in
let w = @fmt-i64 out 0 result in
let _ = @write 1 out w in
let _ = @write 1 "\n" 1 in
0
