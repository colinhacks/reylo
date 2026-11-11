select * from (select id, lower(email) as e from users) s where s.e = :e
