let input = @buf-alloc 16777216 in
let n = @stdin-slurp input 16777216 in
let nls = rec {
  loop = lambda pos. lambda c.
    if @ge pos n then c else
      let p = @scan-byte input pos (@sub n pos) 10 in
      if @ge p n then c else loop (@add p 1) (@add c 1)
} in loop 0 0 in
let total = if @eq n 0 then 0 else
  (if @eq (@buf-get input (@sub n 1)) 10 then nls else @add nls 1) in
let out = @buf-alloc 32 in
let w = @fmt-i64 out 0 total in
let _ = @write 1 out w in
let _ = @write 1 "\n" 1 in
0
