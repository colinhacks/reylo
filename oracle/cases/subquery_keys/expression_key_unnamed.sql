select * from (select lower(email) from users) s where s.lower = :e
