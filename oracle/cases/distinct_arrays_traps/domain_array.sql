select array_agg(distinct a) filter (where id in (1, 2)) as nested from j
