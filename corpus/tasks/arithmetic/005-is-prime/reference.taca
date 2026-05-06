let ibuf = @buf-alloc 16 in
let n = @read 0 ibuf 16 in
let nl = @scan-byte ibuf 0 n 10 in
let v = @parse-i64 ibuf 0 nl in
let p =
  if @lt v 2 then 0
  else if @lt v 4 then 1
  else if @eq (@mod v 2) 0 then 0
  else
    rec { trial = lambda pack.
      let nn = @div pack 50000 in
      let i = @mod pack 50000 in
      if @gt (@mul i i) nn then 1
      else if @eq (@mod nn i) 0 then 0
      else trial (@add (@mul nn 50000) (@add i 2))
    } in trial (@add (@mul v 50000) 3) in
let _ = if p then @write 1 "yes" 3 else @write 1 "no" 2 in
let _ = @write 1 "\n" 1 in
0
