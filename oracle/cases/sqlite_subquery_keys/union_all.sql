select * from (select id from posts union all select id from posts) s where s.id = :id
