select p.title, u.name from posts p join users u on u.id = p.author_id where p.id = :id
