select * from (select p.id, u.name from users u left join posts p on p.author_id = u.id) s where s.id = :id
