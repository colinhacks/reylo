select * from (select id::text as id, title from posts) s where s.id = :id
