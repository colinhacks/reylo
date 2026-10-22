select s.j from users u, lateral (select jsonb_build_object('id', p.id) as j from posts p where p.user_id = u.id) s
