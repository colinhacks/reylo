select n, count(*) as c from f group by rollup (n)
