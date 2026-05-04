let input = @buf-alloc 200002 in
let len = @read 0 input 200002 in
let table = @i64-alloc 200002 in
let n = @token-index-any input 0 len " \n" 2 table in
let _ = @sort-ranges-by-bytes input table n in
let groups = @i64-alloc 300003 in
let g = @count-equal-ranges input table n groups in
let scratch = @buf-alloc 2048 in
let out = @buf-alloc 32 in
rec {emit = lambda i.
  if @ge i g then 0 else
    let off = @i64-get groups (@mul i 3) in
    let l = @i64-get groups (@add (@mul i 3) 1) in
    let c = @i64-get groups (@add (@mul i 3) 2) in
    let _ = @buf-copy scratch 0 input off l in
    let _ = @buf-set scratch l 58 in
    let w = @fmt-i64 scratch (@add l 1) c in
    let _ = @buf-set scratch (@add (@add l 1) w) 10 in
    let _ = @write 1 scratch (@add (@add l 2) w) in
    emit (@add i 1)
} in emit 0
