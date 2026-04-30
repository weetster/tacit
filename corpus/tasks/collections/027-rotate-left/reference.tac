let input = @buf-alloc 1000000 in
let len = @read 0 input 1000000 in
let line_end = @scan-byte input 0 len 10 in
let k_start = @add line_end 1 in
let k_end = @scan-byte input k_start (@sub len k_start) 10 in
let k = @parse-i64 input k_start (@sub k_end k_start) in
let out = @buf-alloc 32 in
let count = rec {
  skip = lambda pos.
    if @ge pos line_end then pos else
      if @eq (@buf-get input pos) 32 then skip (@add pos 1) else pos;
  token_end = lambda pos.
    if @ge pos line_end then line_end else
      if @eq (@buf-get input pos) 32 then pos else token_end (@add pos 1);
  count_tokens = lambda pos. lambda acc.
    let p = skip pos in
    if @ge p line_end then acc else
      let e = token_end p in
      count_tokens (@add e 1) (@add acc 1)
} in count_tokens 0 0 in
let _ = if @eq count 0 then @write 1 "\n" 1 else
  let raw = @mod k count in
  let rot = if @lt raw 0 then @add raw count else raw in
  rec {
    skip = lambda pos.
      if @ge pos line_end then pos else
        if @eq (@buf-get input pos) 32 then skip (@add pos 1) else pos;
    token_end = lambda pos.
      if @ge pos line_end then line_end else
        if @eq (@buf-get input pos) 32 then pos else token_end (@add pos 1);
    emit_int = lambda v. lambda first.
      let _ = if first then 0 else @write 1 " " 1 in
      let w = @fmt-i64 out 0 v in
      @write 1 out w;
    emit_range = lambda pos. lambda idx. lambda first. lambda start. lambda stop.
      let p = skip pos in
      if @ge p line_end then first else
        let e = token_end p in
        let take = if @ge idx start then @lt idx stop else @eq 1 0 in
        let next_first = if take then
          (let v = @parse-i64 input p (@sub e p) in
           let _ = emit_int v first in 0)
        else first in
        emit_range (@add e 1) (@add idx 1) next_first start stop
  } in
  let first = emit_range 0 0 1 rot count in
  let _ = emit_range 0 0 first 0 rot in
  @write 1 "\n" 1 in
0
