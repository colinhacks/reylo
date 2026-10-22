select jsonb_agg(name) as o from team group by rollup (id)
