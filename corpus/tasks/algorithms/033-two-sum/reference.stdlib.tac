let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let target_end = @scan-byte input 0 len 10 in
let target = @parse-i64 input 0 target_end in
let nums_start = @add target_end 1 in
let nums_end = @scan-byte input nums_start (@sub len nums_start) 10 in
let xs = @i64-alloc 100001 in
let out = @buf-alloc 32 in
let n = rec {
  skip = lambda pos.
    if @ge pos nums_end then pos else
      if @eq (@buf-get input pos) 32 then skip (@add pos 1) else pos;
  token_end = lambda pos.
    if @ge pos nums_end then nums_end else
      if @eq (@buf-get input pos) 32 then pos else token_end (@add pos 1);
  load = lambda pos. lambda i.
    let p = skip pos in
    if @ge p nums_end then i else
      let e = token_end p in
      let _ = @i64-set xs i (@parse-i64 input p (@sub e p)) in
      load (@add e 1) (@add i 1)
} in load nums_start 0 in
let result = rec {
  find_j = lambda i. lambda j. lambda vi.
    if @ge j n then -1 else
      let vj = @i64-get xs j in
      if @eq (@add vi vj) target then @add (@mul i 100000) j
      else find_j i (@add j 1) vi;
  find_i = lambda i.
    if @ge i n then -1 else
      let r = find_j i (@add i 1) (@i64-get xs i) in
      if @ge r 0 then r else find_i (@add i 1)
} in find_i 0 in
let _ = if @lt result 0 then @write 1 "-1\n" 3 else
  let i = @div result 100000 in
  let j = @mod result 100000 in
  let w1 = @fmt-i64 out 0 i in
  let _ = @write 1 out w1 in
  let _ = @write 1 " " 1 in
  let w2 = @fmt-i64 out 0 j in
  let _ = @write 1 out w2 in
  @write 1 "\n" 1 in
0
