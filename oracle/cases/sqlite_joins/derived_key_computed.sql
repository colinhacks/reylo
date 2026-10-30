select * from (select id + 0 as id from users) s where s.id = :id
