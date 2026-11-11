update comments c set body = 'x' from users u where c.author_id = u.id returning c.author_id
