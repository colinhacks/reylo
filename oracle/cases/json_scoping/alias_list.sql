select x.a from (select jsonb_build_object('k', 1) as a, '{}'::jsonb as b from team) x (b, a)
