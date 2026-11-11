select * from (select id, title from posts intersect all select id, title from posts) s where s.id = :id
