rec {even = lambda n. if n then odd (@sub n 1) else 1; odd = lambda n. if n then even (@sub n 1) else 0} in even 4
