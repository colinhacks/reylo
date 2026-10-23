select x.o from (guest m cross join lateral (select 1 as o) x) j, team m cross join lateral (select jsonb_build_object('n', m.motto) as o) x
