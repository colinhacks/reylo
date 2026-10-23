select x.o from team t, member m left join lateral (select jsonb_build_object('n', m.first_name, 'motto', t.motto) as o) x on true
