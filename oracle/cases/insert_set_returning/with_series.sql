with i as (insert into p values (generate_series(10, 12), 'x') returning id) select id from i
