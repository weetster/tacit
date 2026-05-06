let input = @buf-alloc 1000002 in
let len = @stdin-slurp input 1000002 in
let lines = @i64-alloc (@add (@mul len 2) 2) in
let n = @line-index input len lines in
let best = rec {
  scan = lambda i. lambda b.
    if @ge i n then b else
      if @gt (@range-len lines i) (@range-len lines b)
      then scan (@add i 1) i
      else scan (@add i 1) b
} in scan 1 0 in
let _ = @write-range 1 input (@range-start lines best) (@range-len lines best) in
let _ = @write 1 "\n" 1 in
0
