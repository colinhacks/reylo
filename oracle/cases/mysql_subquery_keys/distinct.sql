select * from (select distinct author_id from posts) s where s.author_id = :a
