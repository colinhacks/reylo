select * from (select p.id, p.title, u.name from posts p left join users u on u.id = p.author_id) s where s.id = :id
