select l.* from posts p cross join lateral rows from (generate_series(1, p.id::int), generate_series(1, 2)) l(a, b)
