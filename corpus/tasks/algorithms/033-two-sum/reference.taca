let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let target_end = @scan-byte input 0 len 10 in
let target = @parse-i64 input 0 target_end in
let nums_start = @add target_end 1 in
let nums_end = @scan-byte input nums_start (@sub len nums_start) 10 in
let out = @buf-alloc 32 in
let result = rec {
  skip = lambda pos.
    if @ge pos nums_end then pos else
      if @eq (@buf-get input pos) 32 then skip (@add pos 1) else pos;
  token_end = lambda pos.
    if @ge pos nums_end then nums_end else
      if @eq (@buf-get input pos) 32 then pos else token_end (@add pos 1);
  count_tokens = lambda pos. lambda acc.
    let p = skip pos in
    if @ge p nums_end then acc else
      let e = token_end p in
      count_tokens (@add e 1) (@add acc 1);
  value_at = lambda want. lambda pos. lambda idx.
    let p = skip pos in
    let e = token_end p in
    if @eq idx want then @parse-i64 input p (@sub e p)
    else value_at want (@add e 1) (@add idx 1);
  find_j = lambda i. lambda j. lambda vi. lambda count.
    if @ge j count then -1 else
      let vj = value_at j nums_start 0 in
      if @eq (@add vi vj) target then @add (@mul i 100000) j
      else find_j i (@add j 1) vi count;
  find_i = lambda i. lambda count.
    if @ge i count then -1 else
      let vi = value_at i nums_start 0 in
      let r = find_j i (@add i 1) vi count in
      if @ge r 0 then r else find_i (@add i 1) count
} in
let count = count_tokens nums_start 0 in
find_i 0 count in
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
