select * from (select p.id, u.name from posts p join users u on u.id = p.user_id) s where s.id = :id
