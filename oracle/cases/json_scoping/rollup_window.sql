select jsonb_agg(name) over () as o from team group by rollup (name)
