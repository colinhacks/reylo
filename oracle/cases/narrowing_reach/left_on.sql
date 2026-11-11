select c.author_id, u.nickname from comments c left join users u on u.id = c.author_id and u.nickname is not null
