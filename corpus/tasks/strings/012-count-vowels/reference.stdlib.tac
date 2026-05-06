let input = @buf-alloc 1048576 in
let n = @stdin-slurp input 1048576 in
let count = rec {
  loop = lambda i. lambda c.
    if @ge i n then c else
      let b = @ascii-tolower (@buf-get input i) in
      let inc = if @eq b 97 then 1 else
                if @eq b 101 then 1 else
                if @eq b 105 then 1 else
                if @eq b 111 then 1 else
                if @eq b 117 then 1 else 0 in
      loop (@add i 1) (@add c inc)
} in loop 0 0 in
let out = @buf-alloc 32 in
let w = @fmt-i64 out 0 count in
let _ = @write 1 out w in
let _ = @write 1 "\n" 1 in
0
