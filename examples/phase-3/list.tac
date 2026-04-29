rec {
  sum = lambda i.
    if @lt i 5 then
      @add
        (if @eq i 0 then 1 else if @eq i 1 then 2 else if @eq i 2 then 3 else if @eq i 3 then 4 else 5)
        (sum (@add i 1))
    else 0
} in sum 0
