let a = @buf-alloc 2 in
let _ = @buf-set a 0 65 in
let _ = @buf-set a 1 66 in
let b = @buf-alloc 2 in
let _ = @buf-set b 0 65 in
let _ = @buf-set b 1 66 in
@buf-eq a 0 b 0 2
