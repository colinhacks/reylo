select array(select x from rows from (generate_series(1, 2), generate_series(1, 3)) x(a, b)) as arr
