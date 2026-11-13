select u.id, l.j from users u, lateral (select jsonb_build_object('k', u.id, 'c', count(*)) as j from posts p where p.user_id = u.id) l group by rollup (u.id, l.j)
