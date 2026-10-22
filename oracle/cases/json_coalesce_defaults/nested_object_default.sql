select jsonb_build_object('ids', coalesce((select jsonb_agg(p.id) from posts p where p.user_id = u.id), '{}')) as j from users u
