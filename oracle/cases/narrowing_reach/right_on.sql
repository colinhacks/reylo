select c.author_id, u.id as uid from comments c right join users u on u.id = c.author_id
