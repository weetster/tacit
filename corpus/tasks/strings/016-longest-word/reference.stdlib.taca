let input = @buf-alloc 10002 in
let n = @stdin-slurp input 10002 in
let len = if @gt n 0 then
  (if @eq (@buf-get input (@sub n 1)) 10 then @sub n 1 else n)
else 0 in
let packed = rec {
  scan = lambda pos. lambda wstart. lambda wcps. lambda bstart. lambda bblen. lambda bcps.
    if @ge pos len then
      (if @gt wcps bcps
       then @add (@mul wstart 100000) (@sub pos wstart)
       else @add (@mul bstart 100000) bblen)
    else
      let b = @buf-get input pos in
      if @eq b 32 then
        (if @gt wcps bcps
         then scan (@add pos 1) (@add pos 1) 0 wstart (@sub pos wstart) wcps
         else scan (@add pos 1) (@add pos 1) 0 bstart bblen bcps)
      else
        let bl = @mod (@utf8-decode input pos) 8 in
        scan (@add pos bl) wstart (@add wcps 1) bstart bblen bcps
} in scan 0 0 0 0 0 0 in
let _ = @write-range 1 input (@div packed 100000) (@mod packed 100000) in
let _ = @write 1 "\n" 1 in
0
