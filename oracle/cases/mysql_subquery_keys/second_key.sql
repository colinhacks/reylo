select * from (select id, email from users) s where s.email = :e
