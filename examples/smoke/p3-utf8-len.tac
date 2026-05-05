let r1 = @eq (@utf8-len 65) 1 in
let r2 = @eq (@utf8-len 233) 2 in
let r3 = @eq (@utf8-len 20013) 3 in
let r4 = @eq (@utf8-len 128512) 4 in
let r5 = @eq (@utf8-len -1) 0 in
let r6 = @eq (@utf8-len 55296) 0 in
let r7 = @eq (@utf8-len 1114112) 0 in
@add r1 (@add r2 (@add r3 (@add r4 (@add r5 (@add r6 r7)))))
