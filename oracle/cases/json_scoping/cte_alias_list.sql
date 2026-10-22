with w (b, a) as (select jsonb_build_object('k', 1) as a, '{}'::jsonb as b) select w.a from team, w
