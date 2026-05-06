let input = @buf-alloc 20004 in
let n = @stdin-slurp input 20004 in
let nl = @scan-byte input 0 n 10 in
let end1 = if @gt nl n then n else nl in
let start2 = if @ge end1 n then n else @add end1 1 in
let end2 = if @eq n 0 then 0 else
  (if @eq (@buf-get input (@sub n 1)) 10 then @sub n 1 else n) in
let h1 = @i64-alloc 256 in
let h2 = @i64-alloc 256 in
let _ = rec {
  fill = lambda i. lambda end. lambda h.
    if @ge i end then 0 else
      let b = @buf-get input i in
      let _ = @i64-set h b (@add (@i64-get h b) 1) in
      fill (@add i 1) end h
} in
  let _ = fill 0 end1 h1 in
  fill start2 end2 h2 in
let ok = rec {
  cmp = lambda i.
    if @ge i 256 then @eq 1 1 else
      if @eq (@i64-get h1 i) (@i64-get h2 i) then cmp (@add i 1)
      else @eq 1 0
} in cmp 0 in
let _ = if ok then @write 1 "true\n" 5 else @write 1 "false\n" 6 in
0
