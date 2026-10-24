select p.title, s.name from posts p join (select id, name from users) s on s.id = p.author_id where p.id = :id
