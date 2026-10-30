select * from (select * from (select id from users) a) b where b.id = :id
