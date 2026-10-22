select x.o from member m cross join lateral (select jsonb_build_object('t', y.t) as o from lateral (select jsonb_build_object('n', m.first_name) as t) y) x
