let ok = rec {
  loop = lambda forward. lambda reverse. lambda place.
    let buf0 = @buf-alloc 1 in
    let n0 = @read 0 buf0 1 in
    if @eq n0 0 then
      @eq forward reverse
    else
      let b0 = @buf-get buf0 0 in
      if @eq b0 10 then
        @eq forward reverse
      else
        if @lt b0 128 then
          loop (@add (@mul forward 256) b0) (@add reverse (@mul b0 place)) (@mul place 256)
        else
          if @lt b0 224 then
            (let buf1 = @buf-alloc 1 in
             let _ = @read 0 buf1 1 in
             let b1 = @buf-get buf1 0 in
             let cp = @add (@mul (@sub b0 192) 64) (@sub b1 128) in
             loop (@add (@mul forward 256) cp) (@add reverse (@mul cp place)) (@mul place 256))
          else
            if @lt b0 240 then
              (let buf1 = @buf-alloc 1 in
               let _ = @read 0 buf1 1 in
               let b1 = @buf-get buf1 0 in
               let buf2 = @buf-alloc 1 in
               let _ = @read 0 buf2 1 in
               let b2 = @buf-get buf2 0 in
               let cp = @add (@mul (@sub b0 224) 4096) (@add (@mul (@sub b1 128) 64) (@sub b2 128)) in
               loop (@add (@mul forward 256) cp) (@add reverse (@mul cp place)) (@mul place 256))
            else
              (let buf1 = @buf-alloc 1 in
               let _ = @read 0 buf1 1 in
               let b1 = @buf-get buf1 0 in
               let buf2 = @buf-alloc 1 in
               let _ = @read 0 buf2 1 in
               let b2 = @buf-get buf2 0 in
               let buf3 = @buf-alloc 1 in
               let _ = @read 0 buf3 1 in
               let b3 = @buf-get buf3 0 in
               let cp = @add (@mul (@sub b0 240) 262144) (@add (@mul (@sub b1 128) 4096) (@add (@mul (@sub b2 128) 64) (@sub b3 128))) in
               loop (@add (@mul forward 256) cp) (@add reverse (@mul cp place)) (@mul place 256))
} in loop 0 0 1 in
let _ = if ok then @write 1 "yes\n" 4 else @write 1 "no\n" 3 in
0
