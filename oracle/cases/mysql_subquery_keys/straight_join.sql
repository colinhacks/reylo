select p.title, u.name from posts p straight_join users u on u.id = p.author_id where p.id = :id
