select * from (select id, title from posts intersect select id, title from posts) s where s.id = :id
