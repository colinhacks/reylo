with x as (select *, jsonb_build_object('n', name) as o from team) select o from x
