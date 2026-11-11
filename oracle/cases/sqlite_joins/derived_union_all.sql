select * from (select id from users union all select id from users) s where s.id = :id
