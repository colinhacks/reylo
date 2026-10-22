with x as (select jsonb_build_object('a', 1) as o) select (with x as (select '{"b": "s"}'::jsonb as o) select jsonb_build_object('inner', x.o) from x) as v
