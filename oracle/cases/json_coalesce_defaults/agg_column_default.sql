select coalesce((select jsonb_agg(p.id) from posts p where p.user_id = u.id), u.meta) as ids from users u
