select * from (select p.id from posts p full join users u on u.id = p.author_id) s where s.id = :id
