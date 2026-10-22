select coalesce((select jsonb_build_object('id', p.id) from posts p where p.user_id = u.id limit 1), '[]') as j from users u
