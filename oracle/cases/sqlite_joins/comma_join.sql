select p.title, u.name from posts p, users u where u.id = p.user_id and p.id = :id
