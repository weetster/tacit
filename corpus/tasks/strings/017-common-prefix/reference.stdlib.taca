let input = @buf-alloc 1100000 in
let len = @read 0 input 1100000 in
let spans = @i64-alloc 2002 in
let n = @line-index input len spans in
let scratch = @buf-alloc 1002 in
let prefix_len = rec {
  common = lambda bo. lambda bl. lambda keep. lambda j.
    if @ge j keep then keep else
    if @ge j bl then bl else
      if @eq (@buf-get input j) (@buf-get input (@add bo j)) then common bo bl keep (@add j 1) else j;
  shrink = lambda i. lambda keep.
    if @ge i n then keep else
      shrink (@add i 1) (common (@range-start spans i) (@range-len spans i) keep 0)
} in if @eq n 0 then 0 else shrink 1 (@range-len spans 0) in
let _ = @buf-copy scratch 0 input 0 prefix_len in
let _ = @buf-set scratch prefix_len 10 in
let _ = @write 1 scratch (@add prefix_len 1) in
0
