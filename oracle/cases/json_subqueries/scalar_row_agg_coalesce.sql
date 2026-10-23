select users.id, (select coalesce(jsonb_agg(q), '[]') from posts q where q.user_id = users.id) as posts from users
