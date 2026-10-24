select * from (select p.id from posts p cross join users u) s where s.id = :id
