let src = @buf-alloc 1 in
let _ = @buf-set src 0 65 in
let dst = @buf-alloc 1 in
let _ = @buf-set dst 0 90 in
let _ = @buf-copy dst 0 src 0 0 in
@buf-get dst 0
