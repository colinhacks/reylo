select (select jsonb_build_object('s', generate_series(1, 2), 't', generate_series(1, 3)) offset 2 limit 1) as j
