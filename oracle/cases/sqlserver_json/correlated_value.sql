select u.id, (select p.id, u.name as author from posts p where p.user_id = u.id for json path) as posts from users u
