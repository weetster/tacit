let _ = rec {
  loop = lambda at_start.
    let buf = @buf-alloc 1 in
    let n = @read 0 buf 1 in
    if @eq n 0 then
      0
    else
      let byte = @buf-get buf 0 in
      if @eq byte 10 then
        0
      else
        let out = if at_start then
          (if @ge byte 97 then (if @le byte 122 then @sub byte 32 else byte) else byte)
        else
          (if @ge byte 65 then (if @le byte 90 then @add byte 32 else byte) else byte) in
        let _ = @buf-set buf 0 out in
        let _ = @write 1 buf 1 in
        loop (if @eq byte 32 then 1 else 0)
} in loop 1 in
let _ = @write 1 "\n" 1 in
0
