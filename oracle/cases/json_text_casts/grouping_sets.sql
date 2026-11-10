select jsonb_build_object('t', (j::text)::jsonb, 'n', count(*)) as o from doc group by rollup (j)
