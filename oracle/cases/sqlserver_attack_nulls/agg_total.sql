select max(n) as m, sum(n) as s, string_agg(s, ',') as sa from f
