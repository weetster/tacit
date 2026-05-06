let first = rec {
  read_first = lambda pack. lambda len.
    let buf = @buf-alloc 1 in
    let n = @read 0 buf 1 in
    if @eq n 0 then
      @add (@mul pack 100) len
    else
      let byte = @buf-get buf 0 in
      if @eq byte 10 then
        @add (@mul pack 100) len
      else
        read_first (@add (@mul pack 27) (@sub byte 96)) (@add len 1)
} in read_first 0 0 in
let prefix = @div first 100 in
let prefix_len = @mod first 100 in
let result = rec {
  pow = lambda i. lambda acc.
    if @eq i 0 then acc else pow (@sub i 1) (@mul acc 27);
  char_at = lambda pack. lambda len. lambda pos.
    let place = pow (@sub (@sub len pos) 1) 1 in
    @mod (@div pack place) 27;
  trunc = lambda pack. lambda len. lambda keep.
    if @eq keep len then pack else @div pack (pow (@sub len keep) 1);
  process = lambda pack. lambda len.
    let buf = @buf-alloc 1 in
    let n = @read 0 buf 1 in
    if @eq n 0 then
      @add (@mul pack 100) len
    else
      line pack len 0 (@eq 1 1) (@buf-get buf 0);
  line = lambda pack. lambda len. lambda matched. lambda active. lambda byte.
    if @eq byte 10 then
      process (trunc pack len matched) matched
    else
      let code = @sub byte 96 in
      let next_active = if active then (if @lt matched len then @eq code (char_at pack len matched) else @eq 1 0) else @eq 1 0 in
      let next_matched = if next_active then @add matched 1 else matched in
      let buf = @buf-alloc 1 in
      let n = @read 0 buf 1 in
      if @eq n 0 then
        @add (@mul (trunc pack len next_matched) 100) next_matched
      else
        line pack len next_matched next_active (@buf-get buf 0)
} in process prefix prefix_len in
let out = @div result 100 in
let out_len = @mod result 100 in
let pow = if @eq out_len 0 then 0 else rec {
  pow_loop = lambda i. lambda acc.
    if @le i 1 then acc else pow_loop (@sub i 1) (@mul acc 27)
} in pow_loop out_len 1 in
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
} in emit out pow in
let _ = @write 1 "\n" 1 in
0
