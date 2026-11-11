select * from (select id, name from users) s where s.id = :id
