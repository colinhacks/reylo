select s.j from (select '{{1,2}}'::int[] as a) t, lateral (select jsonb_build_object('a', t.a) as j) s
