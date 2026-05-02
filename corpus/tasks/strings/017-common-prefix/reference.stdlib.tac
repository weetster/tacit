let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let spans = @i64-alloc 2002 in
let one = @buf-alloc 1 in
let n = rec {
  line_end = lambda pos.
    @scan-byte input pos (@sub len pos) 10;
  next_line = lambda end.
    if @ge end len then len else @add end 1;
  load = lambda pos. lambda i.
    if @ge pos len then i else
      let e = line_end pos in
      let off = @mul i 2 in
      let _ = @i64-set spans off pos in
      let _ = @i64-set spans (@add off 1) (@sub e pos) in
      load (next_line e) (@add i 1)
} in load 0 0 in
let prefix_len = rec {
  start = lambda i. @i64-get spans (@mul i 2);
  slen = lambda i. @i64-get spans (@add (@mul i 2) 1);
  common = lambda bo. lambda bl. lambda keep. lambda j.
    if @ge j keep then keep else
    if @ge j bl then bl else
      let ab = @buf-get input j in
      let bb = @buf-get input (@add bo j) in
      if @eq ab bb then common bo bl keep (@add j 1) else j;
  shrink = lambda i. lambda keep.
    if @ge i n then keep else
      shrink (@add i 1) (common (start i) (slen i) keep 0)
} in if @eq n 0 then 0 else shrink 1 (@i64-get spans 1) in
let _ = rec {
  emit = lambda i.
    if @ge i prefix_len then 0 else
      let _ = @buf-set one 0 (@buf-get input i) in
      let _ = @write 1 one 1 in
      emit (@add i 1)
} in emit 0 in
let _ = @write 1 "\n" 1 in
0
