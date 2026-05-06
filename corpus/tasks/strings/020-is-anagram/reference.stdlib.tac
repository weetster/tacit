let input = @buf-alloc 20004 in
let n = @stdin-slurp input 20004 in
let nl = @scan-byte input 0 n 10 in
let end1 = if @gt nl n then n else nl in
let start2 = if @ge end1 n then n else @add end1 1 in
let end2 = if @eq n 0 then 0 else
  (if @eq (@buf-get input (@sub n 1)) 10 then @sub n 1 else n) in
let counts = @i64-alloc 256 in
let _ = rec {
  init = lambda i.
    if @ge i 256 then 0 else
      let _ = @i64-set counts i 0 in
      init (@add i 1)
} in init 0 in
let _ = rec {
  fill1 = lambda i.
    if @ge i end1 then 0 else
      let b = @buf-get input i in
      let _ = @i64-set counts b (@add (@i64-get counts b) 1) in
      fill1 (@add i 1);
  fill2 = lambda i.
    if @ge i end2 then 0 else
      let b = @buf-get input i in
      let _ = @i64-set counts b (@sub (@i64-get counts b) 1) in
      fill2 (@add i 1)
} in
  let _ = fill1 0 in
  fill2 start2 in
let ok = rec {
  cmp = lambda i.
    if @ge i 256 then @eq 1 1 else
      if @eq (@i64-get counts i) 0 then cmp (@add i 1)
      else @eq 1 0
} in cmp 0 in
let _ = if ok then @write 1 "true\n" 5 else @write 1 "false\n" 6 in
0
