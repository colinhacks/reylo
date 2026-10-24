select p.title, u.name from posts p join users u on u.code = p.title where p.id = :id
