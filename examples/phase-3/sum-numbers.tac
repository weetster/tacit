let result = rec {
  loop = lambda state.
    let buf = @buf-alloc 1 in
    let n = @read 0 buf 1 in
    if @eq n 0 then
      @add (@div state 100000) (@mod state 100000)
    else
      let byte = @buf-get buf 0 in
      if @eq byte 10 then
        loop (@mul (@add (@div state 100000) (@mod state 100000)) 100000)
      else
        loop (@add (@mul (@div state 100000) 100000) (@add (@mul (@mod state 100000) 10) (@sub byte 48)))
} in loop 0 in
let out = @buf-alloc 32 in
let w = @fmt-i64 out 0 result in
let _ = @write 1 out w in
let _ = @write 1 "\n" 1 in
0
