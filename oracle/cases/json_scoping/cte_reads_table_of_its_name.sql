with snap as (select jsonb_build_object('n', snap.o) as o from snap) select o from snap
