let input = @buf-alloc 65536 in
let n = @stdin-slurp input 65536 in
let len = if @gt n 0 then
  (if @eq (@buf-get input (@sub n 1)) 10 then @sub n 1 else n)
else 0 in
let starts = @i64-alloc 10002 in
let count = rec {
  scan = lambda off. lambda k.
    let _ = @i64-set starts k off in
    if @ge off len then k else
      let bl = @mod (@utf8-decode input off) 8 in
      scan (@add off bl) (@add k 1)
} in scan 0 0 in
let _ = rec {
  emit = lambda i.
    if @lt i 0 then 0 else
      let s = @i64-get starts i in
      let e = @i64-get starts (@add i 1) in
      let _ = @write-range 1 input s (@sub e s) in
      emit (@sub i 1)
} in emit (@sub count 1) in
let _ = @write 1 "\n" 1 in
0
