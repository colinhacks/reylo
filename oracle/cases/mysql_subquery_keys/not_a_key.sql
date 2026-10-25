select s.id, u.email from (select id, author_id from posts) s join users u on u.id = s.author_id where u.email = :e
