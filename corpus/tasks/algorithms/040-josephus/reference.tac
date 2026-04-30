let input = @buf-alloc 64 in
let len = @read 0 input 64 in
let first_end = @scan-byte input 0 len 10 in
let second_start = @add first_end 1 in
let second_end = @scan-byte input second_start (@sub len second_start) 10 in
let n = @parse-i64 input 0 first_end in
let k = @parse-i64 input second_start (@sub second_end second_start) in
let result = rec {
  loop = lambda i. lambda pos.
    if @gt i n then pos else
      loop (@add i 1) (@mod (@add pos k) i)
} in loop 2 0 in
let out = @buf-alloc 32 in
let w = @fmt-i64 out 0 result in
let _ = @write 1 out w in
let _ = @write 1 "\n" 1 in
0
