let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let line_end = @scan-byte input 0 len 10 in
let out = @buf-alloc 32 in
let result = rec {
  skip = lambda pos.
    if @ge pos line_end then pos else
      if @eq (@buf-get input pos) 32 then skip (@add pos 1) else pos;
  token_end = lambda pos.
    if @ge pos line_end then line_end else
      if @eq (@buf-get input pos) 32 then pos else token_end (@add pos 1);
  loop = lambda pos. lambda cur. lambda best.
    let p = skip pos in
    if @ge p line_end then best else
      let e = token_end p in
      let x = @parse-i64 input p (@sub e p) in
      let with_prev = @add cur x in
      let next_cur = if @gt x with_prev then x else with_prev in
      let next_best = if @gt next_cur best then next_cur else best in
      loop (@add e 1) next_cur next_best
} in
let p0 = skip 0 in
let e0 = token_end p0 in
let first = @parse-i64 input p0 (@sub e0 p0) in
loop (@add e0 1) first first in
let w = @fmt-i64 out 0 result in
let _ = @write 1 out w in
let _ = @write 1 "\n" 1 in
0
