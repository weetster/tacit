let buf = @buf-alloc 1 in
let _ = rec {
  loop = lambda at_start.
    let n = @read 0 buf 1 in
    if @eq n 0 then 0 else
      let b = @buf-get buf 0 in
      if @eq b 10 then 0 else
        let _ = @buf-set buf 0 (if at_start then @ascii-toupper b else @ascii-tolower b) in
        let _ = @write 1 buf 1 in
        loop (@eq b 32)
} in loop (@eq 1 1) in
let _ = @write 1 "\n" 1 in
0
