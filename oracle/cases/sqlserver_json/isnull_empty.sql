select u.id, isnull((select p.id from posts p where p.user_id = u.id for json path), '[]') as posts from users u
