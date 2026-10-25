select p.title, u.name from posts p, users u where u.id = p.author_id and p.id = :id
