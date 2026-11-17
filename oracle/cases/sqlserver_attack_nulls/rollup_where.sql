select n, count(*) as c from f where n = 0 group by rollup (n)
