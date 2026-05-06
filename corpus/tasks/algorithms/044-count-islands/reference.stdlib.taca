let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let first_end = @scan-byte input 0 len 10 in
let row_table = @i64-alloc 602 in
let cols = @token-index-any input 0 first_end " " 1 row_table in
let table = @i64-alloc 180002 in
let cells = @token-index-any input 0 len " \n" 2 table in
let out = @buf-alloc 32 in
let result = rec {
  flood = lambda idx.
    if @lt idx 0 then 0 else
    if @ge idx cells then 0 else
      let p = @range-start table idx in
      if @ne (@buf-get input p) 49 then 0 else
        let _ = @buf-set input p 48 in
        let col = @mod idx cols in
        let _ = if @gt col 0 then flood (@sub idx 1) else 0 in
        let _ = if @lt col (@sub cols 1) then flood (@add idx 1) else 0 in
        let _ = if @ge idx cols then flood (@sub idx cols) else 0 in
        let _ = if @lt (@add idx cols) cells then flood (@add idx cols) else 0 in
        0;
  count = lambda idx. lambda acc.
    if @ge idx cells then acc else
      if @eq (@buf-get input (@range-start table idx)) 49 then
        (let _ = flood idx in count (@add idx 1) (@add acc 1))
      else
        count (@add idx 1) acc
} in count 0 0 in
let w = @fmt-i64 out 0 result in
let _ = @write 1 out w in
let _ = @write 1 "\n" 1 in
0
