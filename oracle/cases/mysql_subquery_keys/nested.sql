select * from (select * from (select id, title from posts) a) b where b.id = :id
