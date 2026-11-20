select (select u.id, coalesce((select p.id from posts p where p.user_id = u.id for json path), '[]') as posts from users u for json path) as doc
