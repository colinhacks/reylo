select x.o from (select jsonb_build_object('n', name) as o from team) x group by rollup (x.o)
