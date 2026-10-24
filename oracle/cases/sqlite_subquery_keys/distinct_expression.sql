select * from (select distinct author_id, title || '!' as t from posts) s where s.author_id = :a
