let input = @buf-alloc 400008 in
let n = @stdin-slurp input 400008 in
let len = if @gt n 0 then
  (if @eq (@buf-get input (@sub n 1)) 10 then @sub n 1 else n)
else 0 in
let cps = @i64-alloc 100001 in
let count = rec {
  scan = lambda off. lambda k.
    if @ge off len then k else
      let packed = @utf8-decode input off in
      let _ = @i64-set cps k (@div packed 8) in
      scan (@add off (@mod packed 8)) (@add k 1)
} in scan 0 0 in
let ok = rec {
  check = lambda i. lambda j.
    if @ge i j then @eq 1 1 else
      if @eq (@i64-get cps i) (@i64-get cps j) then check (@add i 1) (@sub j 1)
      else @eq 1 0
} in check 0 (@sub count 1) in
let _ = if ok then @write 1 "yes\n" 4 else @write 1 "no\n" 3 in
0
