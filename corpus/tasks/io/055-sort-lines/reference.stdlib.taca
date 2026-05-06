let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let spans = @i64-alloc (@add (@mul len 2) 2) in
let n = @line-index input len spans in
let _ = @sort-ranges-by-bytes input spans n in
let scratch = @buf-alloc 1000002 in
rec {emit = lambda i.
  if @ge i n then 0 else
    let off = @range-start spans i in
    let l = @range-len spans i in
    let _ = @buf-copy scratch 0 input off l in
    let _ = @buf-set scratch l 10 in
    let _ = @write 1 scratch (@add l 1) in
    emit (@add i 1)
} in emit 0
