select * from (select name from users) s where s.name = :n
