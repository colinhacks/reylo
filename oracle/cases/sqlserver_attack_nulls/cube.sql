select n, v, count(*) as c from f group by cube (n, v)
