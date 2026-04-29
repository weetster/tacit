let ibuf = @buf-alloc 16 in
let n = @read 0 ibuf 16 in
let nl = @scan-byte ibuf 0 n 10 in
let v = @parse-i64 ibuf 0 nl in
let sq = if @eq v 0 then 0 else
  rec { isqrt = lambda pack.
    let vv = @div pack 40000 in
    let g = @mod pack 40000 in
    let ng = @div (@add g (@div vv g)) 2 in
    if @ge ng g then g else isqrt (@add (@mul vv 40000) ng)
  } in isqrt (@add (@mul v 40000) 32768) in
let pr1 = rec { out_small = lambda pack.
  let vv = @div pack 80000 in
  let ir = @mod pack 80000 in
  let i = @div ir 2 in
  let pr = @mod ir 2 in
  if @gt (@mul i i) vv then pr
  else if @eq (@mod vv i) 0 then
    (if @lt i vv then
      (let _ = if pr then @write 1 " " 1 else 0 in
       let obuf = @buf-alloc 32 in
       let w = @fmt-i64 obuf 0 i in
       let _ = @write 1 obuf w in
       out_small (@add (@mul vv 80000) (@add (@mul (@add i 1) 2) 1)))
    else out_small (@add (@mul vv 80000) (@add (@mul (@add i 1) 2) pr)))
  else out_small (@add (@mul vv 80000) (@add (@mul (@add i 1) 2) pr))
} in out_small (@add (@mul v 80000) 2) in
let _ = rec { out_large = lambda pack.
  let vv = @div pack 80000 in
  let ir = @mod pack 80000 in
  let i = @div ir 2 in
  let pr = @mod ir 2 in
  if @eq i 0 then pr
  else if @eq (@mod vv i) 0 then
    (let q = @div vv i in
     if @gt q i then
       (if @lt q vv then
         (let _ = if pr then @write 1 " " 1 else 0 in
          let obuf = @buf-alloc 32 in
          let w = @fmt-i64 obuf 0 q in
          let _ = @write 1 obuf w in
          out_large (@add (@mul vv 80000) (@add (@mul (@sub i 1) 2) 1)))
       else out_large (@add (@mul vv 80000) (@add (@mul (@sub i 1) 2) pr)))
     else out_large (@add (@mul vv 80000) (@add (@mul (@sub i 1) 2) pr)))
  else out_large (@add (@mul vv 80000) (@add (@mul (@sub i 1) 2) pr))
} in out_large (@add (@mul v 80000) (@add (@mul sq 2) pr1)) in
let _ = @write 1 "\n" 1 in
0
