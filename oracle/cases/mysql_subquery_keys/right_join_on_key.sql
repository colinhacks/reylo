select p.title, u.name from users u right join posts p on u.id = p.author_id where p.id = :id
