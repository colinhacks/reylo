select n, count(*) as c from f group by grouping sets ((n), ())
