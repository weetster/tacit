let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let line_end = @scan-byte input 0 len 10 in
let target_start = @add line_end 1 in
let target_end = @scan-byte input target_start (@sub len target_start) 10 in
let target = @parse-i64 input target_start (@sub target_end target_start) in
let out = @buf-alloc 32 in
let result = rec {
  skip = lambda pos.
    if @ge pos line_end then pos else
      if @eq (@buf-get input pos) 32 then skip (@add pos 1) else pos;
  token_end = lambda pos.
    if @ge pos line_end then line_end else
      if @eq (@buf-get input pos) 32 then pos else token_end (@add pos 1);
  count_tokens = lambda pos. lambda acc.
    let p = skip pos in
    if @ge p line_end then acc else
      let e = token_end p in
      count_tokens (@add e 1) (@add acc 1);
  value_at = lambda want. lambda pos. lambda idx.
    let p = skip pos in
    let e = token_end p in
    if @eq idx want then @parse-i64 input p (@sub e p)
    else value_at want (@add e 1) (@add idx 1);
  search = lambda lo. lambda hi.
    if @ge lo hi then -1 else
      let mid = @div (@add lo hi) 2 in
      let v = value_at mid 0 0 in
      if @eq v target then mid else
      if @lt v target then search (@add mid 1) hi else search lo mid
} in
let n = count_tokens 0 0 in
search 0 n in
let w = @fmt-i64 out 0 result in
let _ = @write 1 out w in
let _ = @write 1 "\n" 1 in
0
