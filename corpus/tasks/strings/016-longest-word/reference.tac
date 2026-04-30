let packed = rec {
  loop = lambda current. lambda current_len. lambda best. lambda best_len.
    let buf = @buf-alloc 1 in
    let n = @read 0 buf 1 in
    if @eq n 0 then
      (if @gt current_len best_len then @add (@mul current 100) current_len else @add (@mul best 100) best_len)
    else
      let byte = @buf-get buf 0 in
      if @eq byte 10 then
        (if @gt current_len best_len then @add (@mul current 100) current_len else @add (@mul best 100) best_len)
      else
        if @eq byte 32 then
          (if @gt current_len best_len then loop 0 0 current current_len else loop 0 0 best best_len)
        else
          loop (@add (@mul current 27) (@sub byte 96)) (@add current_len 1) best best_len
} in loop 0 0 0 0 in
let word = @div packed 100 in
let len = @mod packed 100 in
let pow = if @eq len 0 then 0 else rec {
  pow_loop = lambda i. lambda acc.
    if @le i 1 then acc else pow_loop (@sub i 1) (@mul acc 27)
} in pow_loop len 1 in
let _ = rec {
  emit = lambda pack. lambda place.
    if place then
      (let digit = @div pack place in
       let rest = @mod pack place in
       let buf = @buf-alloc 1 in
       let _ = @buf-set buf 0 (@add digit 96) in
       let _ = @write 1 buf 1 in
       emit rest (@div place 27))
    else 0
} in emit word pow in
let _ = @write 1 "\n" 1 in
0
