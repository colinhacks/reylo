select x.o from member m cross join lateral (select jsonb_build_object('n', m.first_name) as o) x group by rollup (x.o)
