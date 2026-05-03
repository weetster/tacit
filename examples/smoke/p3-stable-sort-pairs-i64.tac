let keys = @i64-alloc 4 in
let values = @i64-alloc 4 in
let _ = @i64-set keys 0 2 in
let _ = @i64-set values 0 20 in
let _ = @i64-set keys 1 1 in
let _ = @i64-set values 1 10 in
let _ = @i64-set keys 2 2 in
let _ = @i64-set values 2 21 in
let _ = @i64-set keys 3 1 in
let _ = @i64-set values 3 11 in
let _ = @stable-sort-pairs-i64 keys values 4 in
let ok0 = @add (@eq (@i64-get keys 0) 1) (@eq (@i64-get values 0) 10) in
let ok1 = @add (@eq (@i64-get keys 1) 1) (@eq (@i64-get values 1) 11) in
let ok2 = @add (@eq (@i64-get keys 2) 2) (@eq (@i64-get values 2) 20) in
let ok3 = @add (@eq (@i64-get keys 3) 2) (@eq (@i64-get values 3) 21) in
@add (@add ok0 ok1) (@add ok2 ok3)
