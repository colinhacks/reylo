select generate_series(1, 2) as g group by generate_series(1, 2), generate_series(1, 3)
