let input = @buf-alloc 4096 in
let out = @buf-alloc 32 in
let total = rec {
  count_chunk = lambda i. lambda n. lambda acc. lambda last.
    if @ge i n then @add (@mul acc 256) last else
      let byte = @buf-get input i in
      let next_acc = if @eq byte 10 then @add acc 1 else acc in
      count_chunk (@add i 1) n next_acc byte;
  loop = lambda acc. lambda saw. lambda last.
    let n = @stdin-slurp input 4096 in
    if @eq n 0 then
      (if @eq saw 1 then
        (if @eq last 10 then acc else @add acc 1)
      else 0)
    else
      let packed = count_chunk 0 n 0 last in
      let count = @div packed 256 in
      let next_last = @mod packed 256 in
      loop (@add acc count) 1 next_last
} in loop 0 0 0 in
let w = @fmt-i64 out 0 total in
let _ = @write 1 out w in
let _ = @write 1 "\n" 1 in
0
