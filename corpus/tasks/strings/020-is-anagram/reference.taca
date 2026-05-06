let ok = rec {
  same = lambda len1. lambda a1. lambda a2. lambda a3. lambda a4. lambda len2. lambda b1. lambda b2s. lambda b3s. lambda b4.
    if @eq len1 len2 then
      (if @eq a1 b1 then
        (if @eq a2 b2s then
          (if @eq a3 b3s then
            @eq a4 b4
          else @eq 1 0)
        else @eq 1 0)
      else @eq 1 0)
    else @eq 1 0;
  first = lambda len. lambda sum1. lambda sum2. lambda sum3. lambda sum4.
    let buf = @buf-alloc 1 in
    let n = @read 0 buf 1 in
    if @eq n 0 then
      second len sum1 sum2 sum3 sum4 0 0 0 0 0
    else
      let byte = @buf-get buf 0 in
      if @eq byte 10 then
        second len sum1 sum2 sum3 sum4 0 0 0 0 0
      else
        let b2 = @mul byte byte in
        let b3 = @mul b2 byte in
        first (@add len 1) (@add sum1 byte) (@add sum2 b2) (@add sum3 b3) (@add sum4 (@mul b3 byte));
  second = lambda len1. lambda a1. lambda a2. lambda a3. lambda a4. lambda len2. lambda b1. lambda b2s. lambda b3s. lambda b4.
    let buf = @buf-alloc 1 in
    let n = @read 0 buf 1 in
    if @eq n 0 then
      same len1 a1 a2 a3 a4 len2 b1 b2s b3s b4
    else
      let byte = @buf-get buf 0 in
      if @eq byte 10 then
        same len1 a1 a2 a3 a4 len2 b1 b2s b3s b4
      else
        let c2 = @mul byte byte in
        let c3 = @mul c2 byte in
        second len1 a1 a2 a3 a4 (@add len2 1) (@add b1 byte) (@add b2s c2) (@add b3s c3) (@add b4 (@mul c3 byte))
} in first 0 0 0 0 0 in
let _ = if ok then @write 1 "true\n" 5 else @write 1 "false\n" 6 in
0
