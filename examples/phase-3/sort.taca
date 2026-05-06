let buf = @buf-alloc 5 in
let _ = @buf-set buf 0 5 in
let _ = @buf-set buf 1 3 in
let _ = @buf-set buf 2 1 in
let _ = @buf-set buf 3 4 in
let _ = @buf-set buf 4 2 in
let key = @buf-get buf 1 in
let _ = if @lt key (@buf-get buf 0) then
  (let _ = @buf-set buf 1 (@buf-get buf 0) in
   @buf-set buf 0 key)
else 0 in
let key = @buf-get buf 2 in
let _ = if @lt key (@buf-get buf 1) then
  (let _ = @buf-set buf 2 (@buf-get buf 1) in
   if @lt key (@buf-get buf 0) then
     (let _ = @buf-set buf 1 (@buf-get buf 0) in
      @buf-set buf 0 key)
   else @buf-set buf 1 key)
else 0 in
let key = @buf-get buf 3 in
let _ = if @lt key (@buf-get buf 2) then
  (let _ = @buf-set buf 3 (@buf-get buf 2) in
   if @lt key (@buf-get buf 1) then
     (let _ = @buf-set buf 2 (@buf-get buf 1) in
      if @lt key (@buf-get buf 0) then
        (let _ = @buf-set buf 1 (@buf-get buf 0) in
         @buf-set buf 0 key)
      else @buf-set buf 1 key)
   else @buf-set buf 2 key)
else 0 in
let key = @buf-get buf 4 in
let _ = if @lt key (@buf-get buf 3) then
  (let _ = @buf-set buf 4 (@buf-get buf 3) in
   if @lt key (@buf-get buf 2) then
     (let _ = @buf-set buf 3 (@buf-get buf 2) in
      if @lt key (@buf-get buf 1) then
        (let _ = @buf-set buf 2 (@buf-get buf 1) in
         if @lt key (@buf-get buf 0) then
           (let _ = @buf-set buf 1 (@buf-get buf 0) in
            @buf-set buf 0 key)
         else @buf-set buf 1 key)
      else @buf-set buf 2 key)
   else @buf-set buf 3 key)
else 0 in
let out = @buf-alloc 32 in
let w = @fmt-i64 out 0 (@buf-get buf 0) in
let _ = @write 1 out w in
let _ = @write 1 "\n" 1 in
let w = @fmt-i64 out 0 (@buf-get buf 1) in
let _ = @write 1 out w in
let _ = @write 1 "\n" 1 in
let w = @fmt-i64 out 0 (@buf-get buf 2) in
let _ = @write 1 out w in
let _ = @write 1 "\n" 1 in
let w = @fmt-i64 out 0 (@buf-get buf 3) in
let _ = @write 1 out w in
let _ = @write 1 "\n" 1 in
let w = @fmt-i64 out 0 (@buf-get buf 4) in
let _ = @write 1 out w in
let _ = @write 1 "\n" 1 in
0
