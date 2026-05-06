let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let first_end = @scan-byte input 0 len 10 in
let stride = @add first_end 1 in
let out = @buf-alloc 32 in
rec {
  flood = lambda pos.
    if @lt pos 0 then 0 else
    if @ge pos len then 0 else
    if @ne (@buf-get input pos) 49 then 0 else
      let _ = @buf-set input pos 48 in
      let _ = if @ge pos 2 then
        (if @eq (@buf-get input (@sub pos 1)) 32 then flood (@sub pos 2) else 0)
      else 0 in
      let _ = if @lt (@add pos 2) len then
        (if @eq (@buf-get input (@add pos 1)) 32 then flood (@add pos 2) else 0)
      else 0 in
      let _ = if @ge pos stride then flood (@sub pos stride) else 0 in
      let _ = if @lt (@add pos stride) len then flood (@add pos stride) else 0 in
      0;
  count = lambda pos. lambda acc.
    if @ge pos len then acc else
      if @eq (@buf-get input pos) 49 then
        (let _ = flood pos in count (@add pos 1) (@add acc 1))
      else
        count (@add pos 1) acc
} in
let result = count 0 0 in
let w = @fmt-i64 out 0 result in
let _ = @write 1 out w in
let _ = @write 1 "\n" 1 in
0
