select (select coalesce(jsonb_agg(p), '[]') from posts p where p.id = 1) as j from users
