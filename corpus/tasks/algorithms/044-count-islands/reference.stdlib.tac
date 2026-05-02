let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let first_end = @scan-byte input 0 len 10 in
let grid = @i64-alloc 90001 in
let out = @buf-alloc 32 in
let cols = rec {
  skip = lambda pos.
    if @ge pos first_end then pos else
      if @eq (@buf-get input pos) 32 then skip (@add pos 1) else pos;
  token_end = lambda pos.
    if @ge pos first_end then first_end else
      if @eq (@buf-get input pos) 32 then pos else token_end (@add pos 1);
  count = lambda pos. lambda acc.
    let p = skip pos in
    if @ge p first_end then acc else
      let e = token_end p in
      count (@add e 1) (@add acc 1)
} in count 0 0 in
let cells = rec {
  skip = lambda pos.
    if @ge pos len then pos else
      let b = @buf-get input pos in
      if @eq b 32 then skip (@add pos 1) else
      if @eq b 10 then skip (@add pos 1) else pos;
  token_end = lambda pos.
    if @ge pos len then len else
      let b = @buf-get input pos in
      if @eq b 32 then pos else
      if @eq b 10 then pos else token_end (@add pos 1);
  load = lambda pos. lambda i.
    let p = skip pos in
    if @ge p len then i else
      let e = token_end p in
      let _ = @i64-set grid i (@parse-i64 input p (@sub e p)) in
      load (@add e 1) (@add i 1)
} in load 0 0 in
let result = rec {
  flood = lambda idx.
    if @lt idx 0 then 0 else
    if @ge idx cells then 0 else
    if @ne (@i64-get grid idx) 1 then 0 else
      let _ = @i64-set grid idx 0 in
      let col = @mod idx cols in
      let _ = if @gt col 0 then flood (@sub idx 1) else 0 in
      let _ = if @lt col (@sub cols 1) then flood (@add idx 1) else 0 in
      let _ = if @ge idx cols then flood (@sub idx cols) else 0 in
      let _ = if @lt (@add idx cols) cells then flood (@add idx cols) else 0 in
      0;
  count = lambda idx. lambda acc.
    if @ge idx cells then acc else
      if @eq (@i64-get grid idx) 1 then
        (let _ = flood idx in count (@add idx 1) (@add acc 1))
      else
        count (@add idx 1) acc
} in count 0 0 in
let w = @fmt-i64 out 0 result in
let _ = @write 1 out w in
let _ = @write 1 "\n" 1 in
0
