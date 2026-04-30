let result = rec {
  loop = lambda count.
    let buf = @buf-alloc 1 in
    let n = @read 0 buf 1 in
    if @eq n 0 then
      count
    else
      let byte = @buf-get buf 0 in
      let next = if @eq byte 97 then @add count 1 else
        if @eq byte 101 then @add count 1 else
        if @eq byte 105 then @add count 1 else
        if @eq byte 111 then @add count 1 else
        if @eq byte 117 then @add count 1 else
        if @eq byte 65 then @add count 1 else
        if @eq byte 69 then @add count 1 else
        if @eq byte 73 then @add count 1 else
        if @eq byte 79 then @add count 1 else
        if @eq byte 85 then @add count 1 else
        count in
      loop next
} in loop 0 in
let obuf = @buf-alloc 32 in
let w = @fmt-i64 obuf 0 result in
let _ = @write 1 obuf w in
let _ = @write 1 "\n" 1 in
0
