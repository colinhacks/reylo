select u.id, (select p.id, p.body from posts p where p.user_id = u.id for json path, include_null_values) as posts from users u
