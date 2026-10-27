select * from (select id from posts union select title from posts) s where s.id = :id
