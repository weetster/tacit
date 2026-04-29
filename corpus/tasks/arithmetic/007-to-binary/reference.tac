let ibuf = @buf-alloc 24 in
let n = @read 0 ibuf 24 in
let nl = @scan-byte ibuf 0 n 10 in
let v = @parse-i64 ibuf 0 nl in
let obuf = @buf-alloc 12 in
let w = if @eq v 0 then (let _ = @buf-set obuf 0 48 in 1)
else
let b10 = @mod (@div v 1024) 2 in
let b9 = @mod (@div v 512) 2 in
let b8 = @mod (@div v 256) 2 in
let b7 = @mod (@div v 128) 2 in
let b6 = @mod (@div v 64) 2 in
let b5 = @mod (@div v 32) 2 in
let b4 = @mod (@div v 16) 2 in
let b3 = @mod (@div v 8) 2 in
let b2 = @mod (@div v 4) 2 in
let b1 = @mod (@div v 2) 2 in
let b0 = @mod v 2 in
let sw10 = b10 in
let _ = if sw10 then @buf-set obuf 0 (@add b10 48) else 0 in
let s1 = sw10 in let p1 = sw10 in
let sw9 = if b9 then 1 else s1 in
let _ = if sw9 then @buf-set obuf p1 (@add b9 48) else 0 in
let s2 = sw9 in let p2 = if sw9 then @add p1 1 else p1 in
let sw8 = if b8 then 1 else s2 in
let _ = if sw8 then @buf-set obuf p2 (@add b8 48) else 0 in
let s3 = sw8 in let p3 = if sw8 then @add p2 1 else p2 in
let sw7 = if b7 then 1 else s3 in
let _ = if sw7 then @buf-set obuf p3 (@add b7 48) else 0 in
let s4 = sw7 in let p4 = if sw7 then @add p3 1 else p3 in
let sw6 = if b6 then 1 else s4 in
let _ = if sw6 then @buf-set obuf p4 (@add b6 48) else 0 in
let s5 = sw6 in let p5 = if sw6 then @add p4 1 else p4 in
let sw5 = if b5 then 1 else s5 in
let _ = if sw5 then @buf-set obuf p5 (@add b5 48) else 0 in
let s6 = sw5 in let p6 = if sw5 then @add p5 1 else p5 in
let sw4 = if b4 then 1 else s6 in
let _ = if sw4 then @buf-set obuf p6 (@add b4 48) else 0 in
let s7 = sw4 in let p7 = if sw4 then @add p6 1 else p6 in
let sw3 = if b3 then 1 else s7 in
let _ = if sw3 then @buf-set obuf p7 (@add b3 48) else 0 in
let s8 = sw3 in let p8 = if sw3 then @add p7 1 else p7 in
let sw2 = if b2 then 1 else s8 in
let _ = if sw2 then @buf-set obuf p8 (@add b2 48) else 0 in
let s9 = sw2 in let p9 = if sw2 then @add p8 1 else p8 in
let sw1 = if b1 then 1 else s9 in
let _ = if sw1 then @buf-set obuf p9 (@add b1 48) else 0 in
let s10 = sw1 in let p10 = if sw1 then @add p9 1 else p9 in
let sw0 = if b0 then 1 else s10 in
let _ = if sw0 then @buf-set obuf p10 (@add b0 48) else 0 in
if sw0 then @add p10 1 else p10 in
let _ = @write 1 obuf w in
let _ = @write 1 "\n" 1 in
0
