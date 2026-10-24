select * from (select id + 0 as id, title from posts) s where s.id = :id
