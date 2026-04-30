let base_count = 100001 in
let base_cur = 256 in
let base_started = 25600256 in
let _ = rec {
  loop = lambda state.
    let count = @mod state base_count in
    let s1 = @div state base_count in
    let cur = @mod s1 base_cur in
    let started = @div s1 base_cur in
    let buf = @buf-alloc 1 in
    let n = @read 0 buf 1 in
    if @eq n 0 then
      (if started then
        (let out = @buf-alloc 32 in
         let w = @fmt-i64 out 0 count in
         let _ = @write 1 out w in
         let _ = @buf-set buf 0 cur in
         @write 1 buf 1)
      else
        0)
    else
      let byte = @buf-get buf 0 in
      if @eq byte 10 then
        (if started then
          (let out = @buf-alloc 32 in
           let w = @fmt-i64 out 0 count in
           let _ = @write 1 out w in
           let _ = @buf-set buf 0 cur in
           @write 1 buf 1)
        else
          0)
      else
        if started then
          (if @eq byte cur then
            loop (@add base_started (@add (@mul cur base_count) (@add count 1)))
          else
            (let out = @buf-alloc 32 in
             let w = @fmt-i64 out 0 count in
             let _ = @write 1 out w in
             let _ = @buf-set buf 0 cur in
             let _ = @write 1 buf 1 in
             loop (@add base_started (@add (@mul byte base_count) 1))))
        else
          loop (@add base_started (@add (@mul byte base_count) 1))
} in loop 0 in
let _ = @write 1 "\n" 1 in
0
