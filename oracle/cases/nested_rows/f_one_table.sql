select p.id, (select u from users u where u.id = p.author_id) as author from posts p
