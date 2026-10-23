select jsonb_build_object('n', name) as o from team group by rollup (o)
