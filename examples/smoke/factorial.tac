rec {fact = lambda n. if n then @mul n (fact (@sub n 1)) else 1} in fact 5
