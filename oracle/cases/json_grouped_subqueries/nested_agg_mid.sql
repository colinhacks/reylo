select (select jsonb_build_object('m', (select max(p.id)), 'k', 1) from posts p where false) as j
