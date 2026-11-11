select (select jsonb_build_object('a', jsonb_agg(x)) from (select 1 x where false) s group by rollup (x)) as j
