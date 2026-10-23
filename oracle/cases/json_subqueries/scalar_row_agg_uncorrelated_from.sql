select users.id, (select coalesce(jsonb_agg(q), '[]') from posts q) as posts from users
