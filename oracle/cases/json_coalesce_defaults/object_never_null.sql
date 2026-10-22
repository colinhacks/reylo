select coalesce(jsonb_build_object('id', id), '[]') as j from posts
