let input = @buf-alloc 32 in
let len = @read 0 input 32 in
let line_end = @scan-byte input 0 len 10 in
let n = @parse-i64 input 0 line_end in
let out = @buf-alloc 32 in
let _ = rec {
  loop = lambda i.
    if @gt i n then 0 else
      let _ = if @eq (@mod i 15) 0 then @write 1 "FizzBuzz\n" 9 else
        if @eq (@mod i 3) 0 then @write 1 "Fizz\n" 5 else
        if @eq (@mod i 5) 0 then @write 1 "Buzz\n" 5 else
          let w = @fmt-i64 out 0 i in
          let _ = @write 1 out w in
          @write 1 "\n" 1 in
      loop (@add i 1)
} in loop 1 in
0
