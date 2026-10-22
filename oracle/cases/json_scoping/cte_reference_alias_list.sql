with w as (select jsonb_build_object('k', 1) as a, '{}'::jsonb as b) select y.a from team, w as y (b, a)
