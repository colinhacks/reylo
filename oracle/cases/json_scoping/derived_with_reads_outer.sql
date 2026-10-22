with x as (select '[1]'::jsonb as o) select y.v from (with z as (select 1 as one) select jsonb_build_object('k', x.o) as v from x, z) y
