select users.id, (select coalesce(jsonb_agg(q.id), '[]') from posts q where q.user_id = users.id) as ids from users
