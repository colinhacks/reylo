select users.id, (select coalesce(json_agg(q), '[]'::json) from posts q where q.user_id = users.id) as posts from users
