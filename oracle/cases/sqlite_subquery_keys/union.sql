select * from (select id from posts union select author_id from posts) s where s.id = :id
