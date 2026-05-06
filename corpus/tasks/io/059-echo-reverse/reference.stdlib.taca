let input = @buf-alloc 1000002 in
let n = @stdin-slurp input 1000002 in
rec {
  prev = lambda pos.
    if @lt pos 0 then -1 else
      if @eq (@buf-get input pos) 10 then pos else prev (@sub pos 1);
  loop = lambda end.
    if @lt end 0 then 0 else
      let sp = prev (@sub end 1) in
      let st = @add sp 1 in
      let _ = @write-range 1 input st (@sub end st) in
      let _ = @write 1 "\n" 1 in
      if @lt sp 0 then 0 else loop sp
} in
let last = if @eq n 0 then -1 else
  (if @eq (@buf-get input (@sub n 1)) 10 then @sub n 1 else n) in
loop last
