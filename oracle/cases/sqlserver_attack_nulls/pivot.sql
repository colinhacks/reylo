select p.[0] as z from (select n, id from f) s pivot (max(id) for n in ([0], [1])) p
