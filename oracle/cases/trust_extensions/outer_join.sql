select p.title, u.email, u.age from posts p left join users u on u.id = p.author_id where u.email = :e
