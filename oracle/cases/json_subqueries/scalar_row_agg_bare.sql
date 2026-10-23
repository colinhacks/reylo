select users.id, (select jsonb_agg(q) from posts q where q.user_id = users.id) as posts from users
