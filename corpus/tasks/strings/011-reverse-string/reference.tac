let _ = rec {
  rev = lambda state.
    let buf0 = @buf-alloc 1 in
    let n0 = @read 0 buf0 1 in
    if @eq n0 0 then
      0
    else
      let b0 = @buf-get buf0 0 in
      if @eq b0 10 then
        0
      else
        if @lt b0 128 then
          (let _ = rev 0 in
           @write 1 buf0 1)
        else
          if @lt b0 224 then
            (let buf1 = @buf-alloc 1 in
             let _ = @read 0 buf1 1 in
             let _ = rev 0 in
             let _ = @write 1 buf0 1 in
             @write 1 buf1 1)
          else
            if @lt b0 240 then
              (let buf1 = @buf-alloc 1 in
               let _ = @read 0 buf1 1 in
               let buf2 = @buf-alloc 1 in
               let _ = @read 0 buf2 1 in
               let _ = rev 0 in
               let _ = @write 1 buf0 1 in
               let _ = @write 1 buf1 1 in
               @write 1 buf2 1)
            else
              (let buf1 = @buf-alloc 1 in
               let _ = @read 0 buf1 1 in
               let buf2 = @buf-alloc 1 in
               let _ = @read 0 buf2 1 in
               let buf3 = @buf-alloc 1 in
               let _ = @read 0 buf3 1 in
               let _ = rev 0 in
               let _ = @write 1 buf0 1 in
               let _ = @write 1 buf1 1 in
               let _ = @write 1 buf2 1 in
               @write 1 buf3 1)
} in rev 0 in
let _ = @write 1 "\n" 1 in
0
