select * from (select id, title from posts where author_id = :a) s where s.id = :id
